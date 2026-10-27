# 0001 — Single port serving HTTP/1.1 and HTTP/2

- Status: accepted
- Date: 2026-10-26

## Context

The server must be reachable by browsers (WebSocket) and by native clients (gRPC).
Browsers cannot speak native gRPC. gRPC requires HTTP/2; the classic WebSocket
handshake is an HTTP/1.1 `Upgrade`. HTTP/2-only was considered and rejected:
browsers only use HTTP/2 over TLS, RFC 8441 WebSocket support varies across
browsers, and common proxies forward WebSockets to upstreams over HTTP/1.1.

## Decision

- One TCP port (`CISA_ADDR`, default `127.0.0.1:8080`) accepting HTTP/1.1 and
  HTTP/2 (cleartext prior knowledge), detected per connection by `axum::serve`.
- One axum `Router` with the tonic service merged in:
  - `GET /health` → `200 ok` (plain text, no JSON dependency)
  - `/ws` → WebSocket; `any` method so both HTTP/1.1 `GET` and HTTP/2 `CONNECT`
    (RFC 8441) work
  - `/cisa.v1.AutocompleteService/*` → gRPC
- No TLS in the server for now; terminate TLS at the edge.

## Consequences

- One firewall / load balancer rule. The load balancer must support both HTTP/2
  (gRPC) and WebSocket upgrades on the same port.
- Per-protocol limits are configured per route rather than per listener.
