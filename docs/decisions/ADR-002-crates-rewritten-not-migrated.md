# ADR-002 — The scaffolded crates were rewritten, not migrated

**Status:** Accepted
**Date:** 2026-08-03

## Context

The nested workspace removed in ADR-001 contained sixteen crates. Fourteen were
identical 9-line placeholders:

```rust
//! Quatricmorph module
pub struct Module;
impl Module { pub fn new() -> Self { Self } }
```

Two contained code, and both described something other than what their name means
in ARCHITECTURE.md:

* **`q-source`** implemented *source-code* parsing — `parser.rs` with
  `Parser::parse(&self, _source: &str)` and `loader.rs` with
  `Loader::load(path)`, both `todo!()`. ARCHITECTURE.md §4.1 defines `q-source`
  as the **model source** layer: the `ModelSource` trait, `read_range`, and
  manifests.
* **`q-safetensors`** declared `TensorFile { path: String, headers: Vec<String> }`
  with `load()` as `todo!()`. A SafeTensors header is a JSON object of tensor
  name → dtype, shape, and byte offsets; `Vec<String>` cannot represent it, and
  a `load()` that reads a whole file is the opposite of the byte-range access
  the architecture is built on.

## Decision

Every crate was written from scratch against ARCHITECTURE.md. No placeholder
type survives.

`q-cli` and `q-daemon` had binaries that printed plausible messages without
doing anything (`"Starting daemon..."`, `"Listing models..."`,
`"Running with model: {}"`). Those are exactly the fabricated-success pattern
the task guardrails forbid; both binaries were rewritten to perform real work
and exit non-zero when they cannot.

## Alternatives considered

**Extend the placeholders in place.** Rejected: there was nothing to extend, and
the two crates with content had semantics that would have had to be deleted
first. Migrating a wrong abstraction costs more than writing the right one.

**Keep `q-source` as source-code parsing and add a differently-named crate for
model sources.** Rejected: ARCHITECTURE.md §16 fixes the crate names, and there
is no source-code parsing anywhere in the architecture. The old crate was a
scaffold generated from the word "source", not a design.

## Consequences

* Crate names match ARCHITECTURE.md §16 exactly, with the meanings §4, §5, §7,
  §9, §10, and §13 give them.
* `q-cuda` is added beyond §16's list; see ADR-007.
* No `todo!()` remains on any path a test or the daemon can reach — unbuilt
  functionality returns `QError::NotImplemented` with a requirement ID, so it
  surfaces as a 501 rather than a panic.
