# 0005 — Container image

- Status: accepted
- Date: 2026-10-26

## Context

The front-end lives in another repository and needs a running server without
installing the Rust toolchain.

## Decision

- Multi-stage `Dockerfile`: build with `rust:<version>-slim-trixie`, compiling
  dependencies in their own layer against a stub `src/` so code changes reuse it.
  No BuildKit-only features (cache mounts), because the classic builder is still
  common (e.g. Colima without `buildx`). Run on
  `gcr.io/distroless/cc-debian13:nonroot` (no shell, non-root user).
- The image sets `CISA_ADDR=0.0.0.0:8080` (the binary default stays `127.0.0.1`)
  and `NO_COLOR=1` so logs have no ANSI escapes.
- The binary is PID 1 (exec-form `ENTRYPOINT`), so `SIGTERM` from `docker stop`
  reaches the graceful shutdown path directly.
- `compose.yaml` builds and runs the image on port 8080 for local development.
- No `HEALTHCHECK`: distroless has no shell or `curl`; orchestrators probe `/health`.

## Consequences

- Release builds use `lto = "fat"`, so every image build pays the final link-time
  optimization; only dependency compilation is cached.
- The dependency layer is invalidated by any change to `Cargo.toml`, `Cargo.lock`,
  `build.rs` or `proto/`.
- The front-end still needs this repository checked out to build the image.
  Publishing to a registry (e.g. GHCR from CI) would remove that and needs its own decision.
