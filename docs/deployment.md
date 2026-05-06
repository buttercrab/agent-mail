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

The staging workflow runs automatically on pushes to `main` and can also be run manually. It builds the release binary on the GitHub runner with Cargo/sccache caching, uploads only the binary over SSH, installs it under `/opt/agent-mail-staging`, restarts `agent-mail-server-staging.service`, and runs `scripts/deployed_mcp_smoke.sh` against `https://staging.agent-mail.cc`. The workflow intentionally rejects production paths, the production service name, and the production URL.

`main` is the staging candidate branch. Production is promoted manually from a selected ref or tag after staging is green.

See [Staging setup](staging-setup.md) for the required checklist.

### Production

Production deploys are manual. The workflow checks out the requested ref, builds the release binary on the GitHub runner with Cargo/sccache caching, uploads only the binary over SSH, restarts `agent-mail-server.service`, and runs public MCP/SSE smoke against production.

Required GitHub environment: `production`

Required secrets:

- `PROD_HOST`
- `PROD_SSH_USER`
- `PROD_SSH_KEY`
- `PROD_AGENT_MAIL_TOKEN`
- `PROD_PUBLIC_IP`

Production smoke targets `https://agent-mail.cc` and requires `/health` to report `environment=production`.

## Runtime Requirements

The server host must provide:

- Linux `x86_64` runtime, matching the current GitHub-hosted runner binary build
- a systemd service per environment
- an isolated install root per environment
- `AGENT_MAIL_DATABASE_URL`
- `AGENT_MAIL_BIND`
- `AGENT_MAIL_TOKEN`
- `AGENT_MAIL_ENVIRONMENT`
- `AGENT_MAIL_ALLOWED_ORIGINS`

`AGENT_MAIL_TOKEN` is required. The server must not run unauthenticated.

The current production host is a Lightsail Nano instance. Deploy workflows must continue to build release binaries on GitHub-hosted runners and upload only the compiled `agent-mail-server` binary. Do not build Rust on the Nano host.

Production and staging use separate PostgreSQL databases and roles on the private RDS instance. RDS is not publicly accessible; the app host reaches it through Lightsail VPC peering and a security group scoped to the app host private IP.

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
