# Qit Trust Model

## Roles
- `Local operator`: the person using the served repo on loopback. Operator access is granted implicitly only when the request `Host` is loopback (`localhost`, `127.0.0.1`, `::1`) and `implicit_owner_mode` is enabled.
- `Durable owner`: a repo-persistent account in request-based mode with owner privileges.
- `Durable user`: a repo-persistent account in request-based mode with standard collaboration privileges.
- `Shared-session user`: an authenticated browser session in legacy shared-session mode.

## Authentication Surfaces
- Git Smart HTTP uses one of two explicit contracts:
  - `shared_session`: the startup `SessionCredentials` username/password.
  - `request_based`: the durable repo username plus either the user's password or a personal access token.
- The Web UI uses a same-origin session cookie after a successful login.
- Request-based onboarding is split into two steps: request access by name/email, then redeem a one-time onboarding token to create a username and password.
- Exposed Web UI sessions do not inherit localhost owner privileges just because they originate from the same machine. The trust decision is based on the request host, not client IP alone.

## Credential Handling
- `qit serve` prints the session username and password by default so the operator can share a working clone command immediately.
- `qit serve --hidden-pass` hides the password from stdout and writes it to a local credentials file instead.
- In `shared_session` mode, the Web UI bootstrap payload only includes Git credentials for the localhost operator session.
- In `request_based` mode, passwords, onboarding tokens, and PATs are stored only as verifiers. Raw onboarding tokens and PAT secrets are shown exactly once when issued.

## Authorization Rules
- Local operators and durable owners can manage repository settings, branch rules, auth mode, access requests, user promotion/demotion, revocation, and pull request lifecycle actions.
- Durable users and shared-session users can view state, create pull requests, and participate with comments or reviews, but cannot manage repository settings or perform owner-only pull request actions.
- Git transport authorization is still credential-based. Branch rules and pull-request requirements remain the main integrity controls once a collaborator has Git credentials.
- Request-based sessions re-resolve durable user state on each authenticated request so revocation, demotion, PAT revocation, and setup resets take effect immediately.

## Hardening Rules
- Login attempts are rate-limited per exposed host after repeated failures.
- Request-based mode never falls back to the shared startup credentials for Web UI or Git authentication.
- Approval is not proof of identity by itself. Approval only issues a one-time onboarding token; the account does not become usable until that token is redeemed.
- The domain layer prevents revoking or demoting the last durable owner through normal owner-management flows.
- Branch-rule patterns are restricted to a safe glob subset before they are written into the Git-side enforcement hook.
- Git HTTP request bodies remain size-limited and configurable through the CLI.

## Operational Expectations
- Multiple `qit serve` processes can share one local/public entrypoint through a local supervisor that routes requests by repo path.
- The current Web UI session store remains in-memory and process-local to each repo worker, so signing into one repo path does not sign the browser into another repo on the same shared host.
- Durable request-based auth state lives in repo-persistent workspace metadata alongside repository settings and pull request records.
- Login attempt throttling is scoped by exposed host plus repo path so one repo path cannot lock out another when they share a hostname.
- Manual smoke testing is still required for environment-bound transports such as ngrok and Tailscale because those paths depend on external services and machine state.
