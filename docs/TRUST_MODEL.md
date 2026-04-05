# Qit Trust Model

## Roles
- `Owner`: the operator on localhost. Owner access is granted implicitly only when the request `Host` is loopback (`localhost`, `127.0.0.1`, `::1`) and `implicit_owner_mode` is enabled.
- `User`: an authenticated shared Web UI session. Exposed sessions must log in explicitly and are limited to non-destructive collaboration actions such as viewing, commenting, and reviewing pull requests.

## Authentication Surfaces
- Git Smart HTTP always uses session-scoped Basic Auth credentials.
- The Web UI uses a same-origin session cookie after a successful login.
- Exposed Web UI sessions do not inherit localhost owner privileges just because they originate from the same machine. The trust decision is based on the request host, not client IP alone.

## Credential Handling
- `qit serve` prints the session username and password by default so the operator can share a working clone command immediately.
- `qit serve --hidden-pass` hides the password from stdout and writes it to a local credentials file instead.
- The Web UI bootstrap payload only includes Git credentials for owner sessions. Shared browser sessions can authenticate, but they do not get the credentials echoed back through `/api/bootstrap`.

## Authorization Rules
- Owners can manage repository settings, branch rules, and pull request lifecycle actions.
- Shared users can view state, create pull requests, and participate with comments or reviews, but cannot manage repository settings or perform owner-only pull request actions.
- Git transport authorization is still credential-based. Branch rules and pull-request requirements are the main integrity controls once a collaborator has Git credentials.

## Hardening Rules
- Login attempts are rate-limited per exposed host after repeated failures.
- Branch-rule patterns are restricted to a safe glob subset before they are written into the Git-side enforcement hook.
- Git HTTP request bodies remain size-limited and configurable through the CLI.

## Operational Expectations
- The current Web UI session store is in-memory and process-local.
- Manual smoke testing is still required for environment-bound transports such as ngrok and Tailscale because those paths depend on external services and machine state.
