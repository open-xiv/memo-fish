FROM rust:1.95-slim-bookworm AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src target/release/memo-fish*

COPY src ./src
RUN touch src/main.rs && cargo build --release

FROM rust:1.95-slim-bookworm AS boringtun
RUN cargo install boringtun-cli --version 0.7.* --locked

FROM debian:bookworm-slim

ARG MEMO_VERSION=dev
ARG MEMO_BUILD=unknown
ENV MEMO_VERSION=${MEMO_VERSION} \
    MEMO_BUILD=${MEMO_BUILD}

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates wireguard-tools iproute2 iptables \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder   /build/target/release/memo-fish     /usr/local/bin/memo-fish
COPY --from=boringtun /usr/local/cargo/bin/boringtun-cli  /usr/local/bin/boringtun-cli
COPY entrypoint.sh                                         /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
