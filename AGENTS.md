## Learned User Preferences

- When implementing an attached plan, do not edit the plan file; use existing todos only (mark items in progress, do not recreate the todo list).
- For this workspace’s tooling, prefer a minimal public workflow: single command with path argument, ngrok started automatically, and the tunnel URL shown in output; keep `qit serve` startup output scannable (avoid large banners and dumping internal paths or snapshot metadata unless debugging).
- For `qit` CLI surface, prefer Git command parity (familiar subcommands and flags) over custom verb names where it matches user expectations.
- For `qit` auth UX, show per-session username and password in the terminal by default and include them in suggested `git clone` commands; offer a flag such as `--hidden-pass` when obscuring credentials is preferred.
- For the `qit` Web UI, only localhost may skip the login gate; exposed transports (including Tailscale, ngrok, and LAN) must require Web UI authentication.
- When starting `qit`, show the local Web UI URL in the terminal so operators can open the session that uses the localhost bypass.
- For `qit-webui` Code views, follow GitHub-style browsing: combine the file tree with the main content, render a folder’s README when one exists (not only at repo root), keep folder navigation in the tree workflow, and avoid showing full local filesystem directory names—use repository-relative presentation.
- Prefer compact, icon-forward UI with a strong React icon set and clear file/folder affordances; keep radii and core `Button` padding on shared design tokens rather than one-off values, and place primary repo actions (for example create branch / open pull request) on the same row as the tab navigation to limit layout shift.
- In user-facing branding and documentation, spell the product **Qit** (capital Q), not all-lowercase.

## Learned Workspace Facts

- Cargo workspace `qit` (“Quick Git”): CLI in `qit-cli`; web UI (Vite/React/TypeScript, Tailwind CSS 4, Headless UI, atomic-design layout) in `crates/qit-webui`; sidecar and worktree behavior in `crates/qit-git`; orchestration and `RepoStore` boundaries in `crates/qit-domain`; HTTP smart protocol and transports (e.g. ngrok) in `crates/qit-http` and `crates/qit-transports`; registry/persistence in `crates/qit-storage`; integration tests under `tests/`.
- The `qit-webui` SPA is served on the same origin as Git smart HTTP (alongside the repository URL path).
- Serving a folder that is already a Git worktree root (it contains `.git`) requires `--allow-existing-git`; snapshots skip `.git` metadata, and a first-time serve can infer the checked-out branch from the host repository.
- Public clone URLs include a path segment tied to the served folder name so default `git clone` checkout directory names stay readable.
- The worktree checked-out branch can differ from the exported (served) branch on the sidecar so local work can track one branch while Git clients still receive another (for example serving `main` while developing on a feature branch).
- Release binaries are expected to cover a wide platform matrix, including Linux ARM64, Windows ARM64, and macOS x64 (Intel), in addition to other primary targets.
- Web UI implicit-owner bypass is based on the request `Host` header (`localhost`, `127.0.0.1`, `::1`), not the client IP alone, so tunnel-forwarded requests that still carry a public or tailnet hostname require Web UI login.
- Web UI clone and copy affordances should show the session’s public base URL when one applies (LAN, ngrok, Tailscale), not only loopback addresses.
- Custom Cursor skills in this repository live under `.cursor/skills/<name>/` as a concise `SKILL.md` (frontmatter + instructions) plus a companion `REPORT_TEMPLATE.md` for the full output shape; examples include `web-product-audit`, `security-code-audit`, `cli-product-review`, and `codebase-engineering-review`.
- For `qit-webui` pull request detail (especially merged PRs), file diffs should reflect the PR’s recorded source/target commit range at merge time, not the live diff between branch tips that may have moved since.
