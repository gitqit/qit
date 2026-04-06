# Qit Parity Contract

This document captures the current LocalCollab behavior that `qit` preserves or intentionally changes.

## Preserved Workflows

### Serve

- `qit <path>` publishes a normal folder without requiring a `.git` directory in that folder.
- Startup resolves the folder path, creates or reopens a hidden bare sidecar repository, snapshots the host folder into its checked-out branch, starts Git Smart HTTP, and prints a clone URL plus a local credentials file path.
- `--transport ngrok`, `--transport tailscale`, `--transport lan`, and `--transport local` are first-class options.
- `--local-only` remains a compatibility alias for local transport.
- `--port` keeps the local Git HTTP listener configurable.
- `--branch` sets the exported branch when a workspace is first served and defaults to `main`.
- Serving a folder that already has its own `.git` directory requires explicit opt-in with `--allow-existing-git`.
- Restarting an existing workspace with a different `--branch` is rejected explicitly; callers must use `qit switch <path> <branch>` to change the served branch.

### Collaboration

- Collaborators use normal Git Smart HTTP with `clone`, `fetch`, and `push`.
- Pushes land in the sidecar repository first.
- Fresh clones check out the exported branch.
- Extra branches may exist in the sidecar repository even though the host folder is intentionally single-branch.

### Apply

- `qit apply <path>` fast-forwards the host folder to the latest exported branch in the sidecar repository.
- `qit apply <path> --branch <name>` applies a specific sidecar branch.
- Applying a different branch also updates the checked-out host branch metadata to match the branch now materialized on disk.
- `--auto-apply` attempts to fast-forward the host folder after successful pushes.
- Dirty host state blocks apply behavior, but pushed commits still stay in the sidecar repository.
- Apply remains fast-forward only.

## Preserved Technical Rules

- Workspace identity is derived from the canonical host path using UUID v5.
- Sidecar history is stored outside the host folder in an app-data directory.
- Snapshots honor the host tree's local `.gitignore` rules even when the host folder is not a Git repository.
- When serving an existing Git worktree root, snapshots skip the host repo's `.git` metadata entirely.
- Snapshot walking does not inherit machine-global Git ignore rules from the serving host.
- Snapshot walking includes vendor/build directories when they are part of the published tree.
- Git HTTP keeps using `git http-backend` and uses the configured transport scheme so HTTPS-aware Git behavior survives tunnel termination without trusting arbitrary client headers.

## Intentional Changes

- `qit` replaces the old no-auth default with startup-generated session credentials.
- Authentication uses HTTP Basic Auth so standard Git clients can keep working.
- The session username and password expire when the serving process exits, are written to a local file, and stay hidden from stdout unless `--show-pass` is requested explicitly.
- The Web UI only grants implicit owner access in local-only mode; exposed sessions must authenticate explicitly and do not inherit trust from loopback forwarding.

## Explicit Non-Goals

- The legacy relay, invite, JWT, and Yamux design in `docs/PROTOCOL.md` is not revived in this rewrite.
- `qit` does not silently mutate the host folder on push unless `--auto-apply` is enabled and the host tree is clean.
