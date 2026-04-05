# Qit

Quick Git for normal folders.

`qit` rebuilds the LocalCollab workflow as a Rust workspace:

- publish a folder without adding `.git` to it
- snapshot that folder into a hidden bare sidecar repository
- serve the sidecar over Git Smart HTTP
- protect the session with a startup-generated username and password
- optionally apply pushed commits back into the host folder

## Workspace Layout

- `qit-cli`: `qit` binary
- `crates/qit-domain`: core workspace and session model
- `crates/qit-git`: sidecar Git and `git http-backend` adapter
- `crates/qit-http`: authenticated Git HTTP server
- `crates/qit-storage`: registry and app-data layout
- `crates/qit-transports`: ngrok, tailscale, and local transport adapters
- `tests`: integration and end-to-end suites

## Requirements

- Rust
- Git on `PATH`

For `--transport ngrok`:

```bash
export NGROK_AUTHTOKEN=your_token_here
```

For `--transport tailscale`:

- `tailscale` on `PATH`
- Tailscale running and logged in
- Funnel enabled on the tailnet

## Quick Start

Run locally:

```bash
cargo run --manifest-path Cargo.toml -p qit -- --transport local ./my-app
```

`qit` will:

1. resolve the folder path
2. create or reopen the sidecar repository
3. snapshot the folder into the host folder's checked-out branch
4. start Git Smart HTTP
5. generate a fresh session username and password
6. print the repo URL, a local credentials file path, and a clone command

Clone from another machine or terminal:

```bash
git clone http://127.0.0.1:8080/my-app
```

`qit` hides the password from stdout and omits it from the suggested clone command by default. Pass `--show-pass` if you explicitly want stdout to print the password and embed credentials in the clone command.

## Commands

Serve a folder:

```bash
cargo run -p qit -- ./my-app
```

Choose a transport:

```bash
cargo run -p qit -- --transport ngrok ./my-app
cargo run -p qit -- --transport tailscale ./my-app
cargo run -p qit -- --transport local ./my-app
```

Serve an existing Git worktree root explicitly:

```bash
cargo run -p qit -- --allow-existing-git .
```

Enable auto-apply:

```bash
cargo run -p qit -- --auto-apply ./my-app
```

Apply pushed commits manually:

```bash
cargo run -p qit -- apply ./my-app
cargo run -p qit -- apply ./my-app --branch feature-x
```

Manage sidecar branches for a served folder:

```bash
cargo run -p qit -- branch ./my-app
cargo run -p qit -- branch ./my-app --list "feat*"
cargo run -p qit -- branch ./my-app -v
cargo run -p qit -- branch ./my-app feature-x
cargo run -p qit -- branch ./my-app release main
cargo run -p qit -- branch ./my-app -m feature-x feature-y
cargo run -p qit -- branch ./my-app -M old-name existing-name
cargo run -p qit -- branch ./my-app -c feature-y feature-copy
cargo run -p qit -- branch ./my-app -d feature-y
cargo run -p qit -- checkout ./my-app feature-x
cargo run -p qit -- checkout ./my-app -b scratch main
cargo run -p qit -- checkout ./my-app -B scratch feature-x
cargo run -p qit -- checkout ./my-app -f feature-x
cargo run -p qit -- switch ./my-app feature-x
```

## Security Model

Unlike the current LocalCollab build, `qit` does not default to anonymous access.

- every server session generates a fresh username and password
- authentication uses HTTP Basic Auth so standard Git clients can use normal clone and push commands
- credentials live only for the lifetime of the serving process
- credentials are written to a local file; stdout hides the password by default and the suggested clone command is uncredentialed unless you pass `--show-pass`
- the Web UI enters as owner automatically only in local-only mode; exposed sessions must authenticate explicitly
- pushes still land in the sidecar repository first
- host-folder writes still require `apply` or `--auto-apply`

This is intentionally simple authentication, not a full multi-user permission system.

## Apply Behavior

- pushes always update the sidecar repository first
- manual `apply` fast-forwards the host folder from the sidecar
- manual `apply --branch <name>` also makes `<name>` the checked-out host branch, because that branch now owns the host tree contents
- `checkout` changes the host folder's local branch without changing the sidecar's served default branch
- `switch` changes both the host folder branch and the sidecar's served default branch
- `--auto-apply` only updates the host folder when it is clean relative to the last applied state
- `--auto-apply` skips updates when the host folder is checked out to a different branch than the served branch
- dirty host state blocks apply behavior but does not reject the push
- restarting an existing workspace with `--branch <name>` is rejected; use `qit switch <path> <name>` to change the served branch

## Snapshot Behavior

- snapshots honor the published tree's local `.gitignore` rules even when the host folder is not a Git repository
- snapshots do not inherit machine-global Git ignore rules from the operator's environment
- snapshots include vendor/build directories when they are part of the published tree
- serving an existing Git worktree root requires `--allow-existing-git`; `qit` snapshots the checked-out branch while skipping `.git` metadata
- the sidecar repository still lives outside the host folder, so `qit` does not snapshot sidecar Git metadata into the published tree

## Operational Notes

- Ctrl+C triggers a bounded graceful shutdown for the local Git HTTP server before tunnel teardown
- the default Git HTTP request body limit is 512 MiB; override it with `--max-body-bytes <n>` if you need a different cap

## Git Parity Notes

- `qit branch` supports `-v` / `-vv`, `--list <pattern>`, branch creation from a start point, force rename with `-M`, and branch copy with `-c` / `-C`.
- `qit checkout` supports Git-shaped `-b`, `-B`, `-f`, `--track`, and `--no-track` forms while preserving the `qit` split between the checked-out branch and the served branch.
- `--track` and `--no-track` are accepted for CLI parity, but `qit` does not persist upstream tracking metadata for the served folder.
- Detached checkout, path checkout, and merge-style checkout are rejected explicitly because they do not map safely onto the sidecar-backed host-folder model.

## Migration From LocalCollab

- the legacy relay/invite protocol is not part of this rewrite
- `qit` focuses on the current shipped serve/apply workflow
- the new architecture is split into smaller crates with explicit boundaries for future auth or relay work
- the parity contract used for the rewrite lives in `docs/PARITY.md`

## Tests

Run the full rewrite test suite:

```bash
cargo test --workspace
```
