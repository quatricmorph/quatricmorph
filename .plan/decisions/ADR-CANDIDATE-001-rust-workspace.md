# ADR-CANDIDATE-001 — Rust workspace introduction

## Status

`Decided` — recording an existing decision. Promoted already:
`docs/decisions/ADR-001-workspace-at-repository-root.md`.

## Context

`ARCHITECTURE.md` §16 specifies a `quatricmorph/` top-level directory containing
`crates/`. The repository places the Cargo workspace at the repository root
instead.

## Repository evidence

* `Cargo.toml` at the repository root, `resolver = "2"`, 18 members, `edition
  2021`, `rust-version 1.78`, `license = "MIT OR Apache-2.0"`.
* All 17 crates exist under `crates/` with the names §16 lists.
* `docs/decisions/ADR-001-workspace-at-repository-root.md` records the departure.
* `cargo test --workspace` → 290 passed, verified at commit `5ca434d`.

## Decision required

None. The alternatives are nonviable: 290 passing tests, a CI pipeline, and a
`target/` directory all assume the current root.

## Options

| Option | Viability |
| --- | --- |
| A — workspace at the repository root (current) | Shipped and working |
| B — move to `quatricmorph/` to match §16 literally | Would break every path in CI, docs, and `.cursor/rules/`, for cosmetic conformance |

## Advantages of A

Shorter paths; `cargo` commands work from the repository root without `--manifest-path`;
one `target/`; matches how the repository is actually cloned and opened.

## Disadvantages of A

Diverges from `ARCHITECTURE.md` §16's diagram, which an agent reading only that
document could misinterpret.

## Risks

Low. The divergence is recorded in an accepted ADR and in
[`CURRENT_ARCHITECTURE.md`](../CURRENT_ARCHITECTURE.md) §6.3.

## Recommended default

**A.** No action.

## Tasks affected

None. `QM-0090` may add a one-line note to `ARCHITECTURE.md` §16 pointing at
`ADR-001`, so the diagram is not read as a live instruction.

## Decision deadline

Passed. Recorded for completeness.
