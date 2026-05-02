
# Proxy

A small HTTP key-value service backed by Postgres.

Records are addressed by `(namespace, partition key, range key)` and store an opaque string `data` payload. The HTTP layer is Axum; storage runs through a single blocking actor that owns one Postgres connection.

## Architecture

`Database` wraps a `deadpool-postgres` connection pool over `tokio-postgres`. Each handler `await`s its op directly — there's no actor and no `spawn_blocking`. Concurrent requests run on parallel pooled connections, capped by the pool's max size.

Pool sizing follows `deadpool-postgres` defaults (currently `num_cpus * 4`). Tune via `DATABASE_URL` connection params or by extending `Config` in `backend.rs` if needed.

The pool's TLS layer is `postgres-native-tls`, which uses the platform TLS stack (SChannel on Windows, Secure Transport on macOS, OpenSSL on Linux). TLS is opt-in via the URL's `sslmode` parameter — `tokio-postgres` will skip TLS unless `sslmode` is set, even though a connector is configured.

## Configuration

| Variable       | Required | Description                                  |
|----------------|----------|----------------------------------------------|
| `API_KEY`      | yes      | Value expected in the `Authorization` header.|
| `DATABASE_URL` | yes      | Postgres connection string. Append `?sslmode=require` (or `verify-ca`/`verify-full`) to use TLS. |

## Running

```bash
API_KEY=secret \
DATABASE_URL=postgres://user:pass@localhost/proxy \
cargo run
```

The server listens on `0.0.0.0:8080`.

## HTTP API

All data routes are nested under `/:ns` and require an `Authorization: <API_KEY>` header.

| Method | Path                         | Body | Description                          |
|--------|------------------------------|------|--------------------------------------|
| GET    | `/`                          |  —   | Health check.                        |
| POST   | `/:ns/put/:pk/:sk`           | text | Upsert record. Body must be ≤ 2048 bytes; oversize requests return `413 Payload Too Large`. |
| GET    | `/:ns/get/:pk/:sk`           |  —   | Point lookup. Returns `{ result }`.  |
| DELETE | `/:ns/delete/:pk/:sk`        |  —   | Delete record.                       |
| GET    | `/:ns/query/:pk[?start=<cursor>]` |  —   | Range query, keyset paginated.   |

Page size is 20. Range queries return `{ "result": [...], "next": <cursor>|null }`. To fetch the next page, pass the previous response's `next` value as `?start=<cursor>`. When `next` is `null`, you've reached the end. Omit `start` (or pass an empty string) to start from the beginning.

## Layout

```
src/
  main.rs       # Axum server + env wiring
  lib.rs       # Database — async pass-through to the pool
  backend.rs   # tokio-postgres + deadpool-postgres pool
  types.rs     # Operations, values, errors, HTTP response shims
```
