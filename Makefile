.PHONY: build fmt clippy test real-test mcp-test public-mcp-smoke ci

build:
	cargo build

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings

test:
	cargo test

real-test:
	./scripts/real_postgres_http_test.sh
	./scripts/real_postgres_mcp_test.sh

mcp-test:
	./scripts/real_postgres_mcp_test.sh

public-mcp-smoke:
	./scripts/public_mcp_smoke.sh

ci: fmt clippy test real-test
