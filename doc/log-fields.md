# memo-fish log fields

Per-service inventory layered on top of the universal fields documented in [`memo-docs/standards/observability.md`](../../memo-docs/standards/observability.md). Update this file in the same PR as a code change that adds or renames a field.

## `event` codes

| code | level | site | meaning |
|---|---|---|---|
| `ingest.unauthorized` | debug | `src/api/ingest.rs` | `POST /ingest` rejected because `X-Auth-Key` missing or wrong |
| `ingest.busy` | warn | `src/api/ingest.rs` | channel full; request answered with 429 |
| `ingest.closed` | error | `src/api/ingest.rs` | writer channel closed; request answered with 503. implies the writer task died |
| `download.unauthorized` | debug | `src/api/download.rs` | `GET /download/:date` rejected; same reason as ingest.unauthorized |
| `download.success` | debug | `src/api/download.rs` | served a day file |
| `download.notfound` | debug | `src/api/download.rs` | requested date has no file (pruned or never existed) |
| `download.read_failed` | error | `src/api/download.rs` | open/read failed for a reason other than NotFound |
| `rotate.success` | info | `src/writer.rs` | writer opened a new day file |
| `rotate.failed` | error | `src/writer.rs` | open for the new day failed; data buffered until next tick retries |
| `write.failed` | error | `src/writer.rs` | `write_all` returned an error mid-flush; the batch is dropped |
| `fsync.failed` | warn | `src/writer.rs` | `sync_data` returned an error; data is still in the page cache |
| `prune.deleted` | info | `src/writer.rs` | retention prune removed a day file |
| `prune.failed` | warn | `src/writer.rs` | retention prune hit an error reading or removing a file |
| `writer.startup_failed` | error | `src/writer.rs` | could not create data dir at startup; writer task exits without consuming any records |
| `writer.drained` | info | `src/writer.rs` | writer saw channel close, finished its final flush, and is exiting |
| `shutdown.signal` | info | `src/main.rs` | SIGTERM / SIGINT received |
| `shutdown.done` | info | `src/main.rs` | writer joined; process exiting |

## Custom fields

| key | type | sites | notes |
|---|---|---|---|
| `addr` | string | listener startup logs | the `host:port` the listener bound to |
| `date` | string (`YYYY-MM-DD`) | `download.*`, `rotate.*`, `prune.deleted` | UTC date the day file corresponds to |
| `file` | string | `prune.deleted`, `prune.failed` | filename only, not full path |
| `dir` | string | `writer.startup_failed` | full data dir path |
| `bytes` | int | `write.failed`, `download.success` | byte count of the operation |
| `queue_cap` | int | `ingest.busy` | total channel slots, for context on the 429 |
| `error` | string | any `*.failed` / `*.unauthorized` is silent; only failures carry this | rendered exception text |
