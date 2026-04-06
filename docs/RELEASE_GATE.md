# Qit Release Gate

## Quorum Checkpoints
Every implementation slice should clear the same four lenses before it is considered complete:

1. Engineering: module boundaries are simpler than before, ownership is local, and the change does not increase file-level coupling.
2. Security: trust boundaries, auth behavior, and operator-facing defaults are explicit and minimally exposed.
3. Product: `qit-cli`, `qit-webui`, and `landing` describe the same workflow, branding, and user expectations.
4. QA: the risk-bearing behavior is covered by automated checks or intentionally called out as manual-only.

## Automated Gate
These checks must stay green before merge:

1. `cargo test --workspace --all-targets --locked`
2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
3. `cargo fmt --all --check`
4. `cd crates/qit-webui/frontend && npm run lint && npm run test && npm run build && npm run smoke`
5. `cd landing && npm run lint && npm run build`

## Required Coverage Areas
- Auth and localhost bypass behavior
- Git credential handling and startup output
- Clone/push/apply and pull-request workflows
- Branch-rule enforcement
- Web UI dashboard load and shared-session login flow
- Landing/docs build integrity

## Manual-Only Gate
These checks are required before release candidates when the relevant transport is advertised:

1. Verify ngrok exposure from a second machine or browser profile.
2. Verify Tailscale Funnel exposure and hostname-based Web UI auth.
3. Verify operator handoff of credentials when `--hidden-pass` is used.

## Reject Conditions
- New unaudited trust paths between Web UI auth and Git credentials
- New large file-level choke points without an offsetting simplification
- Product copy that contradicts actual CLI or Web UI behavior
- Tests that only simulate confidence without covering the real boundary being changed
