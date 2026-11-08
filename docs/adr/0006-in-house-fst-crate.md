# 0006 — In-house FST crate behind the `SuggestionEngine` port

- Status: proposed
- Date: 2026-10-28

## Context

Prefix completion over a large query corpus needs a compact, read-only index
with predictable latency. A finite state transducer (FST) fits: shared prefixes
and suffixes are stored once, lookups touch only the bytes of the key, and
outputs can carry ranking information. We want to own the implementation
instead of depending on an existing FST crate.

## Decision

- **Isolated crate.** The FST lives in `crates/fst`, a workspace member.
  It does not depend on `cisa`, tokio or anything async; its only dependency is
  `thiserror`. It is `#![forbid(unsafe_code)]`. Workspace lints apply to it.
  The package is named `fst` and marked `publish = false`; it shadows the
  crates.io `fst` crate, so that crate must never be added as a dependency.
- **Hexagonal placement.** `fst` is infrastructure, not domain.
  `autocomplete/fst_engine/` is the adapter implementing `SuggestionEngine`;
  no `fst` type crosses the port. The port is the thin trait wrapping the
  library, so no extra wrapper trait is added.
- **Internal layout.** `build/` (writing) and `read/` (reading) never import
  each other; both depend only on `format/`, the versioned binary contract.
  `search/` holds algorithms expressed as automata intersected with the FST.
- **Construction.** Incremental minimal acyclic automaton from sorted keys
  (Daciuk et al., 2000) with output pushing (Mihov & Maurel, 2001). Keys must be
  strictly ascending. Nodes are written in post-order, root address in the footer,
  so the builder streams to any `io::Write`. Minimization starts exact
  (`HashMap` registry); a bounded registry is a later, measured tradeoff.
- **Output algebra.** Outputs implement `Output` (`common`, `concat`, `strip`).
  `MinCost` is the tropical semiring (min, +) with cost = `u64::MAX - weight`.
  After pushing, the output read up to a node is the cost of the best key below
  it, so best-first search yields the exact top-k in O(k · L · log k),
  independent of how many keys share the prefix. A plain key → u64 map uses the
  same algebra, so no second implementation exists until another algebra (e.g.
  cost + payload pair) is needed.
- **Format.** `magic "CISAFST\0" | u16 version | u8 output kind | nodes | u64 root`.
  Malformed input returns `FormatError`, never panics. `Fst<D: AsRef<[u8]>>`
  works over `Vec<u8>` now and a memory map later.
- **Extensibility.** New query kinds (Levenshtein automaton for typos) are new
  `Automaton` implementations; builder, format and reader stay untouched.
- **Text lives outside the crate.** The FST is byte-oriented. Normalization is a
  domain rule in `autocomplete/normalize.rs`, shared by the indexer and the
  engine so both fold text identically. Index keys are
  `normalized ‖ 0x00 ‖ surface` (only `normalized` when both are equal), so a
  completion already carries the text shown to the user.
- **Inline search.** `FstEngine` runs top-k inline on the calling task instead
  of `spawn_blocking`, bounded by `max_expansions`. This is a deliberate
  exception to the "CPU-bound work off the runtime" rule and must be confirmed
  by a criterion benchmark before the engine replaces `MockEngine`.
- **Errors never carry key contents.** Keys hold search queries (personal data,
  LGPD), so errors report positions and lengths.

## Current state

Architecture only. Construction, node decoding and traversal are stubs marked
`STUB (docs/adr/0006)`: the builder writes a valid envelope with the root node
only, and every `Fst` answers from the fixed table in
`crates/fst/src/read/fixture.rs`. `main.rs` still wires `MockEngine`.

## Next steps

1. Builder: unfinished-node stack, registry, output pushing; property tests
   against a `BTreeMap` oracle; fuzz the decoder.
2. Compact node encoding and decoder; `get`, `search`, best-first `top_k`;
   criterion benchmarks (`benches/`) for build throughput and top-k latency.
3. Accent folding in `normalize_query`; an offline `cisa-index` binary; load
   the index from `CISA_INDEX_PATH` in `main.rs`.
4. Later, one ADR each: Levenshtein automaton, memory-mapped index, hot reload.

## Consequences

- The compiler enforces the boundary: `fst` cannot reach domain code.
- The crate can be tested, benchmarked and fuzzed on its own, and extracted
  without refactoring.
- The `Dockerfile` copies `crates/` into the dependency layer, so changes to the
  FST crate rebuild that layer.
- We own correctness and performance of a non-trivial data structure; property
  tests, fuzzing and benchmarks are part of the work, not optional.
