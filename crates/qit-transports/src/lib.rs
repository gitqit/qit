use anyhow::{anyhow, Context};
use async_trait::async_trait;
use ngrok::config::ForwarderBuilder;
use ngrok::prelude::{EndpointInfo, TunnelCloser};
use std::process::{Command, Output};
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicTransport {
    Ngrok,
    Tailscale,
    Local,
}

pub struct PublicEndpoint {
    pub label: &'static str,
    pub public_url: Url,
    pub note: String,
    session: Option<Box<dyn TransportSession>>,
}

impl PublicEndpoint {
    pub async fn shutdown(mut self) -> anyhow::Result<()> {
        if let Some(session) = self.session.take() {
            session.shutdown().await?;
        }
        Ok(())
    }
}

#[async_trait]
trait TransportSession: Send {
    async fn shutdown(self: Box<Self>) -> anyhow::Result<()>;
}

struct NgrokSession {
    forwarder: ngrok::forwarder::Forwarder<ngrok::tunnel::HttpTunnel>,
}

#[async_trait]
impl TransportSession for NgrokSession {
    async fn shutdown(mut self: Box<Self>) -> anyhow::Result<()> {
        self.forwarder.close().await.context("close ngrok tunnel")
    }
}

struct TailscaleSession {
    target: String,
}

#[async_trait]
impl TransportSession for TailscaleSession {
    async fn shutdown(self: Box<Self>) -> anyhow::Result<()> {
        run_tailscale(&["funnel", "--yes", &self.target, "off"])
            .map(|_| ())
            .context("disable tailscale funnel")
    }
}

pub async fn expose(transport: PublicTransport, local_url: &Url) -> anyhow::Result<PublicEndpoint> {
    match transport {
        PublicTransport::Local => Ok(PublicEndpoint {
            label: "LOCAL",
            public_url: local_url.clone(),
            note: "Available on this machine's loopback interface only.".into(),
            session: None,
        }),
        PublicTransport::Ngrok => {
            let mut forwarder = ngrok::Session::builder()
                .authtoken_from_env()
                .connect()
                .await
                .map_err(|err| {
                    anyhow!(
                        "{err}\n\nSet NGROK_AUTHTOKEN or use --transport tailscale / --transport local."
                    )
                })?
                .http_endpoint()
                .metadata("qit")
                .listen_and_forward(local_url.clone())
                .await
                .context("ngrok tunnel")?;

            let public = wait_for_ngrok_url(&mut forwarder).await?;
            Ok(PublicEndpoint {
                label: "NGROK",
                public_url: Url::parse(&public).context("parse ngrok public URL")?,
                note: "Use `ngrok-skip-browser-warning` when cloning on free-tier endpoints."
                    .into(),
                session: Some(Box::new(NgrokSession { forwarder })),
            })
        }
        PublicTransport::Tailscale => {
            let target = local_url.as_str().to_string();
            run_tailscale(&["funnel", "--bg", "--yes", &target]).map_err(|err| {
                anyhow!(
                    "{err}\n\nMake sure Tailscale is installed, running, logged in, and Funnel is enabled."
                )
            })?;
            let public = tailscale_public_url()?;
            Ok(PublicEndpoint {
                label: "TAILSCALE",
                public_url: Url::parse(&public).context("parse tailscale public URL")?,
                note: "Public via Tailscale Funnel.".into(),
                session: Some(Box::new(TailscaleSession { target })),
            })
        }
    }
}

async fn wait_for_ngrok_url(
    forwarder: &mut ngrok::forwarder::Forwarder<ngrok::tunnel::HttpTunnel>,
) -> anyhow::Result<String> {
    let mut public = forwarder.url().trim().to_string();
    if public.is_empty() {
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            public = forwarder.url().trim().to_string();
            if !public.is_empty() {
                break;
            }
        }
    }

    if public.is_empty() {
        return Err(anyhow!(
            "ngrok tunnel started but no public URL was returned yet"
        ));
    }

    Ok(normalize_public_url(&public))
}

fn run_tailscale(args: &[&str]) -> anyhow::Result<Output> {
    let output = Command::new("tailscale")
        .args(args)
        .output()
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                anyhow!("`tailscale` CLI not found. Install Tailscale and ensure it is on PATH.")
            } else {
                anyhow!(err)
            }
        })?;

    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("tailscale exited with status {}", output.status)
    };
    Err(anyhow!(details))
}

fn tailscale_public_url() -> anyhow::Result<String> {
    let output = run_tailscale(&["status", "--json"])?;
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse `tailscale status --json`")?;
    let dns_name = parse_tailscale_dns_name(&value)
        .context("could not determine this machine's Tailscale DNS name")?;

    Ok(format!("https://{dns_name}"))
}

fn normalize_public_url(public: &str) -> String {
    let trimmed = public.trim().trim_end_matches('/').to_string();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed
    } else {
        format!("https://{trimmed}")
    }
}

fn parse_tailscale_dns_name(value: &serde_json::Value) -> Option<String> {
    value
        .get("Self")
        .and_then(|self_obj| self_obj.get("DNSName"))
        .and_then(|dns_name| dns_name.as_str())
        .map(|name| name.trim_end_matches('.').to_string())
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{normalize_public_url, parse_tailscale_dns_name};
    use serde_json::json;

    #[test]
    fn normalize_public_url_adds_https_and_trims_slashes() {
        assert_eq!(
            normalize_public_url("demo.ngrok.app/"),
            "https://demo.ngrok.app"
        );
        assert_eq!(
            normalize_public_url("https://demo.ngrok.app/"),
            "https://demo.ngrok.app"
        );
    }

    #[test]
    fn parse_tailscale_dns_name_reads_self_dns_name() {
        let payload = json!({
            "Self": {
                "DNSName": "host.tailnet.ts.net."
            }
        });
        assert_eq!(
            parse_tailscale_dns_name(&payload).as_deref(),
            Some("host.tailnet.ts.net")
        );
    }
}
