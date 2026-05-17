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

# Pin alloy to the same version droplet-hk's memo-alloy and the k3s alloy
# DaemonSet run, so behavior is identical across hosts.
ARG ALLOY_VERSION=1.4.2

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates wireguard-tools iproute2 iptables curl unzip \
 && curl -fsSL -o /tmp/alloy.zip \
      "https://github.com/grafana/alloy/releases/download/v${ALLOY_VERSION}/alloy-linux-amd64.zip" \
 && unzip /tmp/alloy.zip -d /tmp \
 && mv /tmp/alloy-linux-amd64 /usr/local/bin/alloy \
 && chmod +x /usr/local/bin/alloy \
 && rm /tmp/alloy.zip \
 && apt-get purge -y curl unzip \
 && apt-get autoremove -y \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder   /build/target/release/memo-fish     /usr/local/bin/memo-fish
COPY --from=boringtun /usr/local/cargo/bin/boringtun-cli  /usr/local/bin/boringtun-cli
COPY entrypoint.sh                                         /usr/local/bin/entrypoint.sh
COPY config.alloy                                          /etc/alloy/config.alloy
RUN chmod +x /usr/local/bin/entrypoint.sh \
 && mkdir -p /var/log/memo-fish /var/lib/alloy

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
