FROM rust:1 AS builder

WORKDIR /src
COPY . .
RUN cargo build --release -p agent-mail-server

FROM debian:stable-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/agent-mail-server /usr/local/bin/agent-mail-server

USER 65532:65532
EXPOSE 8787

ENTRYPOINT ["agent-mail-server"]
