# cisa

High-performance AI code autocomplete server written in Rust.
Latency is the product: every decision on the request hot path must consider p50/p99.

## Code style

- Functions: 4-20 lines. Split if longer (enforced by `clippy::too_many_lines`).
- Files: under 500 lines. Split by responsibility.
- Cognitive complexity below 15 per function (enforced by `clippy::cognitive_complexity`).
  When a closure or nested `match` pushes a function over the limit, extract it
  into a named function.
- One thing per function, one responsibility per module (SRP).
- Names: specific and unique. Avoid `data`, `handler`, `manager`, `utils`, `helpers`.
  Prefer names that return <5 grep hits in the codebase.
- Types: public signatures are always explicit. Annotate `let` bindings when the
  type is not obvious from the right-hand side (e.g. `parse()`, `collect()`, `into()`).
- No code duplication. Extract shared logic into a function/module.
- String literals used more than once must be extracted as named `const`s.
  Applies to any string with >5 characters that contains non-alphanumeric characters
  (e.g. routes, env var names, format strings, error messages).
- Empty function bodies must contain a comment explaining why they are
  intentionally empty (e.g. `// no-op: reason`).
- Functions must have at most 7 parameters (enforced by `clippy::too_many_arguments`).
  When exceeded, group related parameters into a named struct
  (e.g. `EngineDeps`, `SamplingParams`, `CompletionContext`).
- Early returns over nested `if`s. Max 2 levels of indentation. Prefer `?`,
  `let ... else` and guard clauses.
- Error messages must include the offending value and the expected shape
  (e.g. `"temperature 3.1 out of range 0.0..=2.0"`).
- Traits used as ports are named by capability (`CompletionEngine`, `Tokenizer`,
  `CompletionCache`). Single-method helper traits may use the verb+"-er" form.
- No `unwrap()` outside tests (enforced by `clippy::unwrap_used`). `expect()` only
  for real invariants, with a message saying why it cannot fail.
- `#[allow(...)]` must carry a `reason = "..."` (enforced by `clippy::allow_attributes_without_reason`).
- No `println!`/`eprintln!`/`dbg!` — use `tracing`.
- Language: everything in the repository is written in English — identifiers, comments,
  doc comments, log messages, error messages, test names, commit messages, ADRs and docs.
  This applies even when the conversation with the user is in another language.

## Errors

- Library code (`src/` except `main.rs`) returns typed errors (one error enum per module).
- Generic/dynamic errors only in the composition root (`main.rs`).
- Map errors to HTTP status in exactly one place.
- Never leak internal details (paths, backend stack traces) in HTTP error bodies.

## Unsafe & FFI

- `unsafe` is only allowed inside inference adapters that wrap native libraries
  (e.g. llama.cpp bindings). Never in `api/` or domain code.
- Every `unsafe` block needs a `// SAFETY:` comment stating the invariant
  (enforced by `clippy::undocumented_unsafe_blocks`).
- Expose a safe API from the adapter; FFI types must not cross the port boundary.

## Performance

- Never block the async runtime. CPU-bound inference runs on a dedicated thread
  pool / `spawn_blocking`, never directly inside an `async fn` on a tokio worker.
- Never hold a lock (`Mutex`, `RwLock`) across an `.await`.
- All queues and channels are bounded. Overload must surface as backpressure or
  a fast error (`503`), never unbounded memory growth.
- Requests must be cancellable: when the client disconnects or a newer request
  supersedes it, stop generating tokens.
- Avoid allocations and clones on the hot path. Prefer borrowing, `Arc` for
  shared read-only state, and reusing buffers.
- Do not optimize without measurement. Performance changes must come with a
  `criterion` benchmark (`benches/`) or a profile (`cargo build --profile profiling`)
  showing the gain. `#[inline]` and `unsafe` for speed need that evidence too.
- Track latency of each stage (validation, tokenization, inference, response)
  through `tracing` spans.

## Comments

- Keep existing comments. Don't strip them on refactor — they carry intent and provenance.
- Write WHY, not WHAT. Skip `// increment counter` above `i += 1`.
- Public items must have a doc comment (`///`): one line of intent + an
  `# Examples` block when practical (use `no_run` for examples needing a runtime or model).
- Reference issue numbers / commit SHAs when a line exists because of a specific bug or upstream constraint.

## Tests

- Every change must pass `make check` (runs `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test`).
- Every new function gets a test. Bug fixes get a regression test.
- Tests must be F.I.R.S.T: fast, independent, repeatable, self-validating, timely.
- Unit tests live in a `#[cfg(test)] mod tests` at the bottom of the file.
  HTTP/end-to-end tests live in `tests/` and drive the `Router` via `tower::ServiceExt::oneshot`.

### Naming

Use `<unit>_<scenario>_<expected_result>` in snake_case. The name must read as a
sentence — "When X, it should Y":

```
mock_engine_with_stop_sequence_truncates_text
completions_route_oversized_context_returns_bad_request
config_from_env_invalid_addr_returns_error
```

Never use generic names like `test_error`, `it_works`, `health_ok`.

### Structure — Arrange / Act / Assert

Every non-trivial test must have the three sections with comments:

```rust
#[tokio::test]
async fn mock_engine_with_stop_sequence_truncates_text() {
    // Arrange
    let engine = MockEngine;
    let input = CompletionInput { stop: vec!["comp".into()], ..input_fixture() };

    // Act
    let output = engine.complete(input).await.unwrap();

    // Assert
    assert_eq!(output.text, "_");
}
```

### Isolation

`cargo test` runs tests in parallel by default — keep them independent.
Never mutate process-global state in tests (`std::env::set_var`, current dir,
global subscribers). Inject values instead (e.g. build `Config` directly rather
than setting `CISA_*` vars). If unavoidable, mark the test with `serial_test::serial`
and explain why in a comment.

### Assertions

- Use `assert_eq!(actual, expected)` / `assert_ne!` over `assert!(a == b)`.
- For enums and errors use `assert!(matches!(err, AppError::BadRequest(_)))`.
- `unwrap()` / `expect()` are fine in tests — a panic is a failed test.
  Test helper functions outside `#[test]` need a file-level
  `#![allow(clippy::unwrap_used, reason = "...")]`.

### Parameterized tests

Convert 3 or more tests that cover the same function with different inputs into
a parameterized test with `rstest` and `#[case]`:

```rust
#[rstest]
#[case(-0.1)]
#[case(2.1)]
#[case(f32::NAN)]
fn validate_temperature_out_of_range_returns_bad_request(#[case] value: f32) {
    assert!(matches!(validate_temperature(value), Err(AppError::BadRequest(_))));
}
```

### No real sleeps

Never use `std::thread::sleep` / `tokio::time::sleep` to wait for something.
Use channels, `tokio::sync::Notify`, or cancellation tokens. For timeouts,
debounce and deadlines use `#[tokio::test(start_paused = true)]` and
`tokio::time::advance` so tests are deterministic and instant.

### Mocking

Use `mockall` (`#[automock]`) for ports when a test needs to verify calls or
inject failures. Simple deterministic fakes (like `MockEngine`) are fine for
collaborators whose behavior is the fixture itself.

### Identical method bodies

Two methods on the same type with identical bodies are a code smell (clippy
does not catch it). Make the second delegate to the first.

### Coverage gate

After every implementation run `make coverage` (`cargo llvm-cov`, requires
`cargo install cargo-llvm-cov`). If total line coverage is **below 89%**, inspect
the uncovered lines (`cargo llvm-cov --html`) and add tests until the threshold is met.

Focus new tests on the uncovered paths that carry the most risk: error branches,
edge cases, and business rules — not trivial getters. Do not pad coverage with
low-value assertions just to hit the number.

### What to test

Prioritize: business rules, edge cases (empty prefix / max context / zero tokens /
UTF-8 multibyte boundaries), every error path, concurrency, cancellation and
timeouts, and regressions. High coverage does not mean high quality — a test
that only exercises the happy path has low value.

## Dependencies

- Inject dependencies through constructors / `AppState`, never through globals
  (`static`, `lazy_static`, `OnceLock`) — except the tracing subscriber.
- Wrap third-party libs behind a thin trait owned by this project, especially
  inference backends, tokenizers and caches.
- Adding a dependency requires a reason: prefer crates that are maintained,
  widely used, and don't pull a heavy transitive tree. Disable unused default features.

## Structure

- Hexagonal Architecture: delivery (`api/`) → domain/services → ports (traits) ← adapters.
- `api/` contains only HTTP concerns: routing, (de)serialization, status codes.
  No business logic — validation and orchestration move to a domain service as they grow.
- `main.rs` is the composition root: config, tracing, wiring of adapters, startup.
- Prefer small focused modules over god files.
- Architecture decisions are recorded under `docs/adr/` using sequential files like `0001-short-title.md`.
- When a change introduces a persistent technical decision or tradeoff, update an
  existing ADR or add a new one in `docs/adr/`.
- Root-level ad hoc decision documents are discouraged; decision records belong in `docs/adr/`.
- Predictable paths (grow as modules are added):

```
.
├── src/
│   └── main.rs              # Composition root: config, tracing, DI, startup/shutdown
├── tests/                   # Integration tests
├── benches/                 # Benchmarks
├── docs/adr/                # Architecture Decision Records
├── Makefile
└── Cargo.toml
```

## Port Conventions

- Ports are traits defined in the `mod.rs` of their module (e.g. `engine/mod.rs`).
- Each port describes one capability (`CompletionEngine`, `Tokenizer`, `CompletionCache`).
- Ports require `Debug + Send + Sync + 'static` so they fit in `Arc<dyn Port>` inside `AppState`.
- Adapters live alongside the port as sibling files (`engine/llama.rs`, `engine/mock.rs`).
- Domain types crossing a port are owned by this crate — never expose types from
  axum, serde-only DTOs, or backend libraries through a port.
- Domain and engine modules never import `axum`.

## Current Decisions

- None yet. Add one line per ADR as they are created.

## Formatting

- Use `rustfmt` defaults. Run with `make fmt`.
- Don't discuss style beyond that.

## Logging & API output

- Structured logs via `tracing` with fields (`tracing::info!(latency_ms, model, "...")`),
  not interpolated strings.
- Never log user code (`prefix`, `suffix`) at `info` or above — it is private
  source code. Log sizes and hashes instead.
- HTTP error bodies are JSON: `{ "error": "<message>" }`.
- API changes that break clients require a new versioned route (`/v2/...`) and an ADR.
