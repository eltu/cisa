# cisa

AI-powered search autocomplete server (Google-style query suggestions) focused on low latency.

One port serves HTTP/1.1 and HTTP/2:

| Route | Protocol | Client |
|---|---|---|
| `GET /health` | HTTP | Load balancers, `curl` |
| `/ws` | WebSocket, binary protobuf frames | Browsers |
| `/cisa.v1.AutocompleteService/Session` | gRPC bidirectional stream | Native clients |

The contract is [`proto/cisa/v1/autocomplete.proto`](proto/cisa/v1/autocomplete.proto).
See [`docs/adr/`](docs/adr/) for the session rules (request ids, supersede, cancel).

## Commands

```bash
make check     # fmt + clippy + tests
make run       # optimized build
make coverage  # requires: cargo install cargo-llvm-cov
```

## Configuration

| Variable | Default | Description |
|---|---|---|
| `CISA_ADDR` | `127.0.0.1:8080` | Listen address |
| `CISA_MAX_QUERY_BYTES` | `512` | Max query size |
| `CISA_DEFAULT_LIMIT` | `8` | Suggestions when the client sends `limit = 0` |
| `CISA_MAX_LIMIT` | `10` | Cap on `limit` |
| `CISA_DEFAULT_LOCALE` | `pt-BR` | Locale when the client sends none |
| `RUST_LOG` | `cisa=info` | Log filter |
