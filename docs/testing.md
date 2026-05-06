# Testing

Agent Mail does not use fake green checks. A validation command must exercise the behavior it claims to cover.

## Local Compile And Lint Gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
make test
```

`make test` currently runs `cargo test`. At initial import, the Rust unit test count was zero, so unit tests should not be treated as the only behavioral coverage.

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
