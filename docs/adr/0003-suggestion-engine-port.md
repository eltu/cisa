# 0003 — Suggestion backend behind the `SuggestionEngine` port

- Status: accepted
- Date: 2026-10-26

## Context

The suggestion backend (prefix index, AI model, remote service) is not chosen yet
and will likely change or be combined.

## Decision

- `SuggestionEngine` (`src/autocomplete/mod.rs`) is the port. It receives a
  validated `SuggestQuery` and returns ordered `Suggestion`s.
- Validation, defaults and limits live in `SuggestService`, not in engines.
- `MockEngine` prefix-matches a fixed pt-BR list and can simulate latency, so
  cancellation and supersede paths are testable.
- The engine is chosen only in `main.rs`.

## Consequences

- Transports never know which engine runs.
- The port returns a boxed future (one allocation per call), negligible next to
  real inference.
