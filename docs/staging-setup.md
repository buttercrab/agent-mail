# Staging Setup Checklist

Staging is not complete until this checklist has real evidence.

## Required Infrastructure

- Public URL, recommended: `https://staging.agent-mail.cc`
- Public IP or tunnel endpoint
- Rust server host or container runtime
- PostgreSQL database separate from production
- Bearer token separate from production
- nginx/Cloudflare path equivalent to production
- systemd service `agent-mail-server-staging.service`
- isolated install root `/opt/agent-mail-staging`

## Required Server Environment

```text
AGENT_MAIL_DATABASE_URL=...
AGENT_MAIL_BIND=127.0.0.1:8788
AGENT_MAIL_TOKEN=...
AGENT_MAIL_ENVIRONMENT=staging
AGENT_MAIL_ALLOWED_ORIGINS=https://staging.agent-mail.cc
```

Use a different bind port from production if staging shares the same host.

## Required GitHub Environment

Create GitHub environment:

```text
staging
```

Required secrets:

```text
STAGING_HOST
STAGING_SSH_USER
STAGING_SSH_KEY
STAGING_AGENT_MAIL_TOKEN
STAGING_PUBLIC_IP
```

Required variable:

```text
STAGING_AGENT_MAIL_URL=https://staging.agent-mail.cc
STAGING_SERVICE=agent-mail-server-staging.service
STAGING_INSTALL_ROOT=/opt/agent-mail-staging
STAGING_REMOTE_SOURCE=/tmp/agent-mail-staging-src
STAGING_PRIVATE_PORT=8788
```

## Required Cloudflare/DNS

- `staging.agent-mail.cc` must resolve to the staging edge.
- TLS must be valid for `staging.agent-mail.cc`.
- If nginx terminates a Cloudflare Origin certificate, the certificate must cover `staging.agent-mail.cc`.

## Verification

Run the manual GitHub workflow:

```text
Staging Deploy
```

The workflow must:

- deploy the current repository revision
- restart the staging service
- run `scripts/deployed_mcp_smoke.sh`
- pass real HTTPS MCP/SSE checks against `STAGING_AGENT_MAIL_URL`
- prove `/health` reports `environment=staging`
- prove the raw staging service port is not publicly reachable

Record the workflow URL and smoke project/mail IDs in `docs/progress.md`.
