#!/bin/sh
# memo-fish container entrypoint. brings up the mesh0 WG interface via boringtun
# (userspace WG — fly machines share the kernel, no kmod available), then execs
# the binary so it inherits the network namespace with 10.66.0.4 reachable.
#
# the mesh0 listener is only for /metrics. the public listener (ingest /
# download / status) binds 0.0.0.0:8080 and works independently — if WG fails
# to come up, public ingest still functions, only Prometheus loses its scrape
# target. that's why this script tries to bring mesh0 up but does NOT exit on
# failure: ingest stays online.
#
# all WG material comes from flyctl secrets — no key ever lands in the image.

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
# droplet-us — prometheus scrapes /metrics from here
PublicKey  = ${WG_PEER_US_PUBLIC}
AllowedIPs = 10.66.0.1/32
Endpoint   = 152.53.193.103:51821
PersistentKeepalive = 25

[Peer]
# droplet-hk — for symmetry with the rest of the mesh
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
    echo "WARN: wg-quick up mesh0 failed — public ingest continues, /metrics on mesh0 will be unreachable" >&2
  fi
else
  echo "WARN: WG_PRIVATE_KEY / WG_PEER_*_PUBLIC missing — skipping mesh0; public ingest continues" >&2
  # tell the binary not to even try binding the metrics listener.
  export MEMO_FISH_METRICS_BIND=""
fi

exec /usr/local/bin/memo-fish "$@"
