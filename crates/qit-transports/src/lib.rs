use anyhow::{anyhow, Context};
use async_trait::async_trait;
use local_ip_address::list_afinet_netifas;
use ngrok::config::ForwarderBuilder;
use ngrok::prelude::{EndpointInfo, TunnelCloser};
use std::net::IpAddr;
use std::process::{Command, Output};
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicTransport {
    Ngrok,
    Tailscale,
    Lan,
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
        PublicTransport::Lan => Ok(PublicEndpoint {
            label: "LAN",
            public_url: lan_public_url(local_url)?,
            note: "Available to other devices on the same local network.".into(),
            session: None,
        }),
        PublicTransport::Ngrok => {
            let mut forwarder = ngrok::Session::builder()
                .authtoken_from_env()
                .connect()
                .await
                .map_err(|err| {
                    anyhow!(
                        "{err}\n\nSet NGROK_AUTHTOKEN or use --transport tailscale / --transport lan / --transport local."
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

fn lan_public_url(local_url: &Url) -> anyhow::Result<Url> {
    let host = detect_lan_host()?;
    lan_public_url_with_host(local_url, &host)
}

fn detect_lan_host() -> anyhow::Result<String> {
    let addresses = list_afinet_netifas().context("list network interfaces")?;
    pick_lan_host(addresses.into_iter().map(|(_, addr)| addr))
        .map(|addr| addr.to_string())
        .context("could not determine a LAN address for this machine")
}

fn pick_lan_host(addrs: impl IntoIterator<Item = IpAddr>) -> Option<IpAddr> {
    let mut fallback_ipv4 = None;
    for addr in addrs {
        match addr {
            IpAddr::V4(ipv4) if ipv4.is_loopback() || ipv4.is_link_local() => continue,
            IpAddr::V4(ipv4) if ipv4.is_private() => return Some(IpAddr::V4(ipv4)),
            IpAddr::V4(ipv4) => {
                fallback_ipv4.get_or_insert(ipv4);
            }
            IpAddr::V6(_) => {}
        }
    }
    fallback_ipv4.map(IpAddr::V4)
}

fn lan_public_url_with_host(local_url: &Url, host: &str) -> anyhow::Result<Url> {
    let mut public_url = Url::parse(&format!("http://{host}/")).context("build LAN public URL")?;
    public_url.set_port(local_url.port()).ok();
    public_url.set_path(local_url.path());
    Ok(public_url)
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
    use super::{
        lan_public_url_with_host, normalize_public_url, parse_tailscale_dns_name, pick_lan_host,
    };
    use serde_json::json;
    use std::net::{IpAddr, Ipv4Addr};
    use url::Url;

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

    #[test]
    fn pick_lan_host_prefers_private_ipv4() {
        let host = pick_lan_host([
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 10)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 5)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 25)),
        ]);
        assert_eq!(host, Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 25))));
    }

    #[test]
    fn pick_lan_host_falls_back_to_non_loopback_ipv4() {
        let host = pick_lan_host([
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 5)),
        ]);
        assert_eq!(host, Some(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 5))));
    }

    #[test]
    fn lan_public_url_keeps_port_and_path() {
        let local_url = Url::parse("http://127.0.0.1:8080/repo").unwrap();
        let public_url = lan_public_url_with_host(&local_url, "192.168.1.25").unwrap();
        assert_eq!(public_url.as_str(), "http://192.168.1.25:8080/repo");
    }
}
