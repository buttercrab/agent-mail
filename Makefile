.PHONY: build fmt clippy test real-test mcp-test adapter-test notify-smoke public-mcp-smoke ci

build:
	cargo build --workspace

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --workspace

real-test:
	./scripts/real_postgres_http_test.sh
	./scripts/real_postgres_mcp_test.sh
	./scripts/real_postgres_adapter_test.sh

mcp-test:
	./scripts/real_postgres_mcp_test.sh

adapter-test:
	./scripts/real_postgres_adapter_test.sh

notify-smoke:
	./scripts/notify_adapter_smoke.sh

public-mcp-smoke:
	./scripts/public_mcp_smoke.sh

ci: fmt clippy test real-test
