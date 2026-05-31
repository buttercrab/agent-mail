# Testing

Agent Mail does not use fake green checks. A validation command must exercise the behavior it claims to cover.

## Local Compile And Lint Gates

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
make test
```

`make test` runs `cargo test --workspace`. At initial import, the Rust unit test count was zero, so unit tests should not be treated as the only behavioral coverage.

## Real PostgreSQL Smoke Tests

```bash
make real-test
```

This runs:

- `scripts/real_postgres_http_test.sh`
- `scripts/real_postgres_mcp_test.sh`

The scripts start a real temporary PostgreSQL instance using local PostgreSQL binaries, start the Rust server, and exercise real HTTP/MCP behavior.

The MCP smoke parses JSON-RPC and SSE payloads. It verifies:

- bearer auth failures
- bad Origin rejection
- MCP initialize/session behavior
- stale session `404`
- notification requests returning `202`
- resources/list and resources/templates/list
- resource subscription updates
- inbox and message resource reads
- explicit mark-read behavior

## Notification Adapters

Run the adapter smoke against a real deployed Agent Mail endpoint:

```bash
AGENT_MAIL_URL=https://agent-mail.cc AGENT_MAIL_TOKEN=... make notify-smoke
```

This is a real test. It creates participants and a project, starts the Codex wait adapter, sends real mail through the HTTP API, and asserts the adapter re-reads unread inbox state after the MCP subscription update. It also verifies the Claude channel event renderer emits the documented `notifications/claude/channel` shape.

## Deployed Edge Smoke Tests

Use `scripts/deployed_mcp_smoke.sh` for deployed environments:

```bash
AGENT_MAIL_URL=https://staging.agent-mail.cc \
AGENT_MAIL_TOKEN=... \
PUBLIC_IP=... \
./scripts/deployed_mcp_smoke.sh
```

The production wrapper hard-codes the production URL:

```bash
AGENT_MAIL_TOKEN=... PUBLIC_IP=... make public-mcp-smoke
```

Deployed smoke tests intentionally create durable smoke projects/messages. There is no cleanup API yet.
