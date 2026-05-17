#!/bin/sh
# WG failure is non-fatal: public ingest stays online, only /metrics on mesh0 is lost.
set -eu

CONF=/etc/wireguard/mesh0.conf

if [ -n "${WG_PRIVATE_KEY:-}" ] && \
   [ -n "${WG_PEER_US_PUBLIC:-}" ] && \
   [ -n "${WG_PEER_HK_PUBLIC:-}" ]; then
  umask 0077
  mkdir -p /etc/wireguard
  cat >"$CONF" <<EOF
[Interface]
PrivateKey = ${WG_PRIVATE_KEY}
Address    = 10.66.0.4/24

[Peer]
PublicKey  = ${WG_PEER_US_PUBLIC}
AllowedIPs = 10.66.0.1/32
Endpoint   = 152.53.193.103:51821
PersistentKeepalive = 25

[Peer]
PublicKey  = ${WG_PEER_HK_PUBLIC}
AllowedIPs = 10.66.0.2/32
Endpoint   = 34.92.75.71:51821
PersistentKeepalive = 25
EOF

  export WG_QUICK_USERSPACE_IMPLEMENTATION=boringtun-cli
  export WG_SUDO=1
  if wg-quick up "$CONF"; then
    echo "mesh0 up on 10.66.0.4"
    MESH0_UP=1
  else
    echo "WARN: wg-quick up mesh0 failed" >&2
  fi
else
  echo "WARN: WG_* env missing — skipping mesh0" >&2
  export MEMO_FISH_METRICS_BIND=""
fi

# log shipper. only run when mesh0 is up — the Loki NodePort lives on
# 10.66.0.1 which is unreachable otherwise. on a WG failure path the
# Rust app still writes to /var/log/memo-fish; alloy's absence just
# means those logs never leave the container (flyctl logs still works).
if [ "${MESH0_UP:-0}" = "1" ] && [ -n "${MEMO_FISH_LOG_DIR:-}" ]; then
  /usr/local/bin/alloy run \
    --storage.path=/var/lib/alloy \
    --server.http.listen-addr=127.0.0.1:12345 \
    /etc/alloy/config.alloy &
  echo "alloy started, shipping ${MEMO_FISH_LOG_DIR}/app.*.log → 10.66.0.1:31100"
fi

exec /usr/local/bin/memo-fish "$@"
