# Contributing

Agent Mail is infrastructure. Prefer small, verifiable changes.

## Requirements

- Rust stable with `rustfmt` and `clippy`
- PostgreSQL for real smoke tests
- `bash`, `curl`, and `python3`

## Local Checks

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
make test
make real-test
```

Do not describe `make test` as behavioral coverage while it has zero Rust unit tests.

## Pull Requests

Every PR should include:

- what changed
- why it changed
- validation commands and real outcomes
- deployment impact, if any
- docs updates for user-visible behavior

Avoid placeholder tests. If something is not tested, say so plainly.
