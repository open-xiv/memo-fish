# memo-fish

Crowdsourced 5-float ingest service. Accepts `POST /ingest` with a 5-element f32 vector, appends server-stamped JSON lines to a daily file on a Fly Volume, and serves the day's NDJSON back via `GET /download/:date`.

Standards: [`memo-docs/standards/observability.md`](../memo-docs/standards/observability.md) for log shape, [`memo-docs/standards/secrets.md`](../memo-docs/standards/secrets.md) for env-var conventions, [`memo-docs/standards/code-style.md`](../memo-docs/standards/code-style.md) for commenting.

## Endpoints

| method | path | network | auth | purpose |
|---|---|---|---|---|
| POST | `/ingest` | public `:8080` | `X-Auth-Key: $MEMO_FISH_INGEST_KEY` | enqueue one record |
| GET  | `/download/:date` | public `:8080` | `X-Auth-Key: $MEMO_FISH_DOWNLOAD_KEY` | stream `data-YYYY-MM-DD.jsonl` |
| GET  | `/status` | public `:8080` | none | full check body |
| GET  | `/status/live` | public `:8080` | none | liveness probe |
| GET  | `/status/ready` | public `:8080` | none | readiness probe |
| GET  | `/metrics` | mesh0 `10.66.0.4:9091` | none | Prometheus text format |

`/ingest` accepts JSON `{"v":[f32; 5]}`. The server stamps `ts` (unix millis, UTC) and appends `{"ts":<i64>,"v":[f32; 5]}\n` to the current day's file. Channel back-pressure surfaces as `429`; channel closed (writer dead) surfaces as `503`.

`/download/:date` returns `application/x-ndjson`. `:date` must match `YYYY-MM-DD` literally. Missing file is `404`. Files older than `MEMO_FISH_RETENTION_DAYS` have been pruned and will 404.

## Local dev

```bash
cargo run
# in another shell
curl -X POST http://127.0.0.1:8080/ingest \
  -H 'content-type: application/json' \
  -H "x-auth-key: ${MEMO_FISH_INGEST_KEY}" \
  -d '{"v":[1.0,2.0,3.0,4.0,5.0]}'

curl http://127.0.0.1:8080/download/$(date -u +%F) \
  -H "x-auth-key: ${MEMO_FISH_DOWNLOAD_KEY}" -o today.jsonl
```

Set `MEMO_FISH_METRICS_BIND=""` for local dev so the service skips the mesh0 listener.

## Deploy

CI on push to `main` builds & pushes `ghcr.io/open-xiv/memo-fish:sha-<sha>` then runs `flyctl deploy --image ...` on app `memo-fish` (region `nrt`, `shared-cpu-1x` 512 MB, 5 GB volume mounted at `/data`).

First-time bootstrap:

```bash
fly apps create memo-fish
fly volumes create data --size 5 --region nrt
fly secrets set \
  MEMO_FISH_INGEST_KEY="$(openssl rand -hex 32)" \
  MEMO_FISH_DOWNLOAD_KEY="$(openssl rand -hex 32)" \
  WG_PRIVATE_KEY="${WG_MESH0_FLY_FISH_PRIVKEY}" \
  WG_PEER_US_PUBLIC="${WG_MESH0_NETCUP_PUBKEY}" \
  WG_PEER_HK_PUBLIC="${WG_MESH0_GCP_PUBKEY}"
fly deploy
```

Adding memo-fish to mesh0 requires the 4-peer expansion in memo-ops (droplet-us and droplet-hk each get a new `[Peer]` entry for `10.66.0.4`). The mesh0 listener will fail to bind until that is rolled out.

## Capacity

5 GB volume + 7-day retention = ~700 MB/day budget. At ~70 bytes/record (`{"ts":1715900000000,"v":[1.0,2.0,3.0,4.0,5.0]}\n`) that's ~10M records/day, ~115 records/sec sustained. Single machine, no horizontal scale (volume is single-attached).

## Archival

Out of repo. The download endpoint is public + auth-keyed, so any external mover can pull `GET /download/<yesterday-utc>` on its own schedule and stash the result. If the puller stays offline longer than `MEMO_FISH_RETENTION_DAYS`, that data is gone.
