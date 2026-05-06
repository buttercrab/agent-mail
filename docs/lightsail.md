# Agent Mail on Lightsail

## Region

Use `us-west-2` for the Lightsail deployment.

## Runtime

- Server: Rust `agent-mail-server`
- Database: private AWS RDS PostgreSQL
- Public endpoint: `https://agent-mail.cc`
- Interfaces:
  - HTTPS JSON API with bearer-token authentication
  - MCP Streamable HTTP at `https://agent-mail.cc/mcp`

## Lightsail Shape

```text
Lightsail Nano instance, us-west-2
  agent-mail-server on 127.0.0.1:8787
  agent-mail-server-staging on 127.0.0.1:8788
  nginx for HTTPS and reverse proxy
  systemd units

RDS PostgreSQL, us-west-2
  private, not publicly accessible
  separate production and staging databases/users
  encrypted storage, automated backups, manual snapshots
```

Cloudflare proxies `agent-mail.cc` and `staging.agent-mail.cc` to the Lightsail static IPv4. nginx terminates Cloudflare Origin CA certificates on `443`, redirects `80` to HTTPS, proxies production to `127.0.0.1:8787`, and proxies staging to `127.0.0.1:8788`.

The `/mcp` nginx location must disable proxy buffering and use long read/send timeouts so the MCP SSE `GET /mcp` stream can deliver `notifications/resources/updated` promptly.

Production proxies to `127.0.0.1:8787`:

```nginx
location /mcp {
    proxy_pass http://127.0.0.1:8787;
    proxy_http_version 1.1;
    proxy_buffering off;
    proxy_cache off;
    proxy_read_timeout 3600s;
    proxy_send_timeout 3600s;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Staging must use the staging service port, not the production port:

```nginx
location /mcp {
    proxy_pass http://127.0.0.1:8788;
    proxy_http_version 1.1;
    proxy_buffering off;
    proxy_cache off;
    proxy_read_timeout 3600s;
    proxy_send_timeout 3600s;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

Open only SSH, HTTP, and HTTPS on the instance firewall. Do not expose ports `8787`, `8788`, or PostgreSQL publicly unless a temporary administrative maintenance window requires it.

The RDS security group must allow `tcp/5432` only from the active Lightsail app host private IP. A previous app host private IP may be kept temporarily during an explicit rollback window, but must be removed after the old host is retired.

Do not rely on a snapshot downsize from a larger Lightsail bundle to Nano. The current production approach is a fresh Nano instance configured with the existing binary-only deploy shape, then static IPv4 reassignment after direct-origin health and RDS checks pass.

## Server Environment

```bash
export AGENT_MAIL_DATABASE_URL='postgres://USER:PASSWORD@HOST:5432/DBNAME?sslmode=require'
export AGENT_MAIL_BIND='127.0.0.1:8787'
export AGENT_MAIL_TOKEN='replace-with-random-token'
export AGENT_MAIL_ENVIRONMENT='production'
export AGENT_MAIL_URL='https://agent-mail.cc'
export AGENT_MAIL_ALLOWED_ORIGINS='https://agent-mail.cc'
```

`AGENT_MAIL_TOKEN` is required. The server must not start in production without it.

Run:

```bash
/opt/agent-mail/bin/agent-mail-server \
  --database-url "$AGENT_MAIL_DATABASE_URL" \
  --bind "$AGENT_MAIL_BIND" \
  --token "$AGENT_MAIL_TOKEN"
```

## Validation Gate

Before deploying, run:

```bash
make build
make test
make real-test
```

`make test` is a Rust compile/unit-test gate. The meaningful end-to-end gate is `make real-test`.

`make real-test` starts a real temporary PostgreSQL cluster, starts the Rust HTTP server, verifies bearer auth, sends cross-project mail, reads full message bodies, marks one project read, and verifies the other project remains unread.

The same gate also runs the MCP smoke test. That test verifies unauthorized MCP requests fail, initializes two MCP sessions, starts participants through MCP tools, subscribes to an inbox resource, receives a real SSE resource-update notification after send, reads inbox/message MCP resources, marks the message read, and confirms the inbox is empty.

After deployment, verify the public endpoint:

```bash
curl -fsS https://agent-mail.cc/health
```

Also verify public MCP behavior through the Cloudflare/nginx edge, not only localhost. A valid public MCP smoke must cover:

- `POST /mcp` initialize with `MCP-Session-Id` response header
- `agent_mail_start` and `agent_mail_project_add`
- `resources/subscribe` for `agent-mail://projects/{alias}/inbox?identity={identity}`
- `GET /mcp` SSE receiving `notifications/resources/updated`
- `resources/read` for inbox and full message body
- `agent_mail_mark_read` followed by an empty inbox resource

Run the public smoke script:

```bash
make public-mcp-smoke
```

## Codex Client Install

Install by URL:

```bash
codex mcp add agent-mail --url https://agent-mail.cc/mcp --bearer-token-env-var AGENT_MAIL_TOKEN
```

The Codex process must inherit `AGENT_MAIL_TOKEN`. Restart Codex after changing MCP configuration or environment.
