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
  else
    echo "WARN: wg-quick up mesh0 failed" >&2
  fi
else
  echo "WARN: WG_* env missing — skipping mesh0" >&2
  export MEMO_FISH_METRICS_BIND=""
fi

exec /usr/local/bin/memo-fish "$@"
