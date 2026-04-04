## Learned User Preferences

- When implementing an attached plan, do not edit the plan file; use existing todos only (mark items in progress, do not recreate the todo list).
- For this workspace’s tooling, prefer a minimal public workflow: single command with path argument, ngrok started automatically, and the tunnel URL shown in output.
- For `qit` CLI surface, prefer Git command parity (familiar subcommands and flags) over custom verb names where it matches user expectations.

## Learned Workspace Facts

- Cargo workspace `qit` (“Quick Git”): CLI in `qit-cli`; sidecar and worktree behavior in `crates/qit-git`; orchestration and `RepoStore` boundaries in `crates/qit-domain`; HTTP smart protocol and transports (e.g. ngrok) in `crates/qit-http` and `crates/qit-transports`; registry/persistence in `crates/qit-storage`; integration tests under `tests/`.
- Public clone URLs include a path segment tied to the served folder name so default `git clone` checkout directory names stay readable.
- The worktree checked-out branch can differ from the exported (served) branch on the sidecar so local work can track one branch while Git clients still receive another (for example serving `main` while developing on a feature branch).
