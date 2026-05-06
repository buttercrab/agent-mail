# Deployment

Agent Mail deploys as a Rust binary behind nginx/Cloudflare with PostgreSQL.

## Environments

### Staging

Staging must be a real deployed environment, not a documentation fiction.

Required GitHub environment: `staging`

Required secrets:

- `STAGING_HOST`
- `STAGING_SSH_USER`
- `STAGING_SSH_KEY`
- `STAGING_AGENT_MAIL_TOKEN`
- `STAGING_PUBLIC_IP`

Required variables:

- `STAGING_AGENT_MAIL_URL`, for example `https://staging.agent-mail.cc`

The staging workflow deploys over SSH, restarts `agent-mail-server.service`, and runs `scripts/deployed_mcp_smoke.sh` against the staging public URL.

### Production

Production deploys are manual.

Required GitHub environment: `production`

Required secrets:

- `PROD_HOST`
- `PROD_SSH_USER`
- `PROD_SSH_KEY`
- `PROD_AGENT_MAIL_TOKEN`
- `PROD_PUBLIC_IP`

Production smoke targets `https://agent-mail.cc`.

## Runtime Requirements

The server host must provide:

- Rust toolchain for current source-build deploys
- systemd service named `agent-mail-server.service`
- `/opt/agent-mail/src`
- `/opt/agent-mail/bin`
- `AGENT_MAIL_DATABASE_URL`
- `AGENT_MAIL_BIND`
- `AGENT_MAIL_TOKEN`
- `AGENT_MAIL_ALLOWED_ORIGINS`

`AGENT_MAIL_TOKEN` is required. The server must not run unauthenticated.

Set `AGENT_MAIL_ALLOWED_ORIGINS` to the public HTTPS origin for each environment. Examples:

```text
AGENT_MAIL_ALLOWED_ORIGINS=https://staging.agent-mail.cc
AGENT_MAIL_ALLOWED_ORIGINS=https://agent-mail.cc
```

## Public Edge Requirements

The `/mcp` route must support long-lived SSE:

- `proxy_buffering off`
- long read/send timeouts
- no public exposure of port `8787`

See [Lightsail notes](lightsail.md) for the current production shape.
