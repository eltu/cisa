# 0004 — Protobuf codegen with protox

- Status: accepted
- Date: 2026-10-26

## Context

`tonic-prost-build` normally shells out to the `protoc` binary, which is not
installed on the development machine and would be another CI prerequisite.

## Decision

`build.rs` compiles `proto/cisa/v1/autocomplete.proto` with `protox` (a pure Rust
protobuf compiler) and passes the descriptor set to `tonic_prost_build::compile_fds`.
Generated code is included through `cisa::proto` with project lints disabled.

## Consequences

- `cargo build` works with only the Rust toolchain.
- `protox` must keep up with protobuf language features we use; plain proto3 is fine.
