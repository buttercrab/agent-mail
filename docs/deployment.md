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
- `STAGING_SERVICE=agent-mail-server-staging.service`
- `STAGING_INSTALL_ROOT=/opt/agent-mail-staging`
- `STAGING_REMOTE_SOURCE=/tmp/agent-mail-staging-src`
- `STAGING_PRIVATE_PORT=8788`

The staging workflow deploys over SSH to `/opt/agent-mail-staging`, restarts `agent-mail-server-staging.service`, and runs `scripts/deployed_mcp_smoke.sh` against `https://staging.agent-mail.cc`. The workflow intentionally rejects production paths, the production service name, and the production URL.

The workflow is manual until real staging infrastructure and GitHub environment secrets are configured. After the first successful manual staging run, it can be changed to run automatically on pushes to `main`.

See [Staging setup](staging-setup.md) for the required checklist.

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
- a systemd service per environment
- an isolated install root per environment
- `AGENT_MAIL_DATABASE_URL`
- `AGENT_MAIL_BIND`
- `AGENT_MAIL_TOKEN`
- `AGENT_MAIL_ENVIRONMENT`
- `AGENT_MAIL_ALLOWED_ORIGINS`

`AGENT_MAIL_TOKEN` is required. The server must not run unauthenticated.

Set `AGENT_MAIL_ALLOWED_ORIGINS` to the public HTTPS origin for each environment. Examples:

```text
AGENT_MAIL_ALLOWED_ORIGINS=https://staging.agent-mail.cc
AGENT_MAIL_ALLOWED_ORIGINS=https://agent-mail.cc
```

Set `AGENT_MAIL_ENVIRONMENT=staging` for staging and `AGENT_MAIL_ENVIRONMENT=production` for production. `/health` exposes this non-secret value so deployed smoke tests can reject the wrong environment.

## Public Edge Requirements

The `/mcp` route must support long-lived SSE:

- `proxy_buffering off`
- long read/send timeouts
- no public exposure of the private service port, for example `8787` in production or `8788` in same-host staging

See [Lightsail notes](lightsail.md) for the current production shape.
