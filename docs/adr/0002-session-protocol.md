# 0002 — Session protocol shared by gRPC and WebSocket

- Status: accepted
- Date: 2026-10-26

## Context

Search autocomplete fires a request per keystroke and almost every request is
obsolete before it finishes. Opening a request per keystroke wastes handshakes
and makes server-side cancellation unreliable.

## Decision

- A single bidirectional RPC: `rpc Session(stream ClientMessage) returns (stream ServerMessage)`.
- Envelope: `ClientMessage { request_id, oneof { suggest, cancel } }` and
  `ServerMessage { request_id, oneof { result, error } }`. `request_id` is a
  client-generated, increasing `uint64`.
- Supersede: a new `suggest` aborts the request in flight. At most one request
  runs per session.
- Cancelled and superseded requests receive no reply. Cancelling an unknown or
  finished id is ignored. Reusing an id still in flight returns `INVALID_ARGUMENT`.
- The WebSocket carries the same messages as binary protobuf frames. Text frames
  close the socket with 1003; undecodable frames with 1007; shutdown with 1001.
  gRPC uses `INVALID_ARGUMENT` / `UNAVAILABLE` statuses for the same cases.
- The session logic is implemented once (`autocomplete/session.rs` +
  `transport/session_driver.rs`); each protocol only implements `SessionTransport`.

## Consequences

- One contract for every client; the front-end generates TypeScript from the same `.proto`.
- Binary frames are not human-readable in browser DevTools.
- Long-lived connections pin a client to one instance: good for future per-session
  caching, but load balancing becomes per connection.
- A reply may still arrive for a request superseded at the very moment it finished;
  clients must ignore replies whose `request_id` is not the latest.
