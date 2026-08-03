# ADR-001 — The workspace lives at the repository root, not in a nested `quatricmorph/`

**Status:** Accepted
**Date:** 2026-08-03

## Context

At the start of this pass the repository contained two competing trees:

* the git repository root (`/`), holding `ARCHITECTURE.md`, `AGENTS.md`,
  `docs/`, and `mm/`;
* a nested `quatricmorph/` directory holding a Cargo workspace with the sixteen
  crate names of ARCHITECTURE.md §16 (all 9-line `pub struct Module` stubs), a
  Vite app, `architectures/`, `schemas/`, `gpu/`, and `python/`.

ARCHITECTURE.md §16 shows a tree rooted at `quatricmorph/`, which reads
ambiguously: it could mean a subdirectory, or it could name the repository
itself. The repository is already named `Quatricmorph`.

Four files in the repository resolve the ambiguity in the same direction:

* `AGENTS.md:27` — *"Target layout: `crates/`, `apps/web`, `apps/desktop`,
  `architectures/`, `schemas/`, `fixtures/` per architecture §16"*, listed among
  root-level paths.
* `AGENTS.md:29` — *"`quatricmorph/` | Legacy Three.js experiment — not the
  architecture target; do not expand as product path"*.
* `docs/requirements/PREREQUISITES.md:58` — *"Legacy `quatricmorph/` Three.js
  tree not treated as architecture target"*, checked done.
* `docs/requirements/VIZ_MVP.md:63` and `docs/TESTING.md:35` — both name the
  legacy `quatricmorph/` tree as out of scope for product architecture.

## Decision

The Cargo workspace, `apps/`, `architectures/`, `schemas/`, `gpu/`, `fixtures/`,
`tests/`, and `docs/` live at the **git repository root**. The nested
`quatricmorph/` directory was consolidated into it and removed.

Content was moved rather than recreated where it had value:

| From | To |
| --- | --- |
| `quatricmorph/apps/web/` | `apps/web/matrix-workspace/` |
| `quatricmorph/architectures/` | `architectures/` |
| `quatricmorph/schemas/` | `schemas/` |
| `quatricmorph/gpu/` | `gpu/` |
| `quatricmorph/python/` | `python/` |
| `quatricmorph/crates/` | *not moved* — see below |

## Alternatives considered

**Keep the workspace at `quatricmorph/`.** The nested `Cargo.toml` already
listed the right sixteen crate names, so this was the smaller edit. Rejected:
it directly contradicts four checked-in documents, and it leaves the repository
with a `Quatricmorph/quatricmorph/` path that reads as an accident.

**Keep both trees, with the nested one as legacy.** Rejected: two Cargo
workspaces with identical crate names in one repository is a standing source of
"which `q-catalog` did I just edit". The task instruction is explicit that a
thing conflicting with ARCHITECTURE.md should be rewritten, redirected, or
**removed**.

**Move the crates too.** Rejected: all sixteen were 9-line placeholders, and two
had actively wrong semantics — `q-source` implemented *source-code* parsing
(`Parser`, `Loader`) rather than *model source* access, and `q-safetensors`
declared `TensorFile { headers: Vec<String> }`. Carrying those forward would
have meant deleting them anyway. They were rewritten from scratch; see ADR-002.

## Consequences

* One workspace root, one `cargo test --workspace`, one `apps/web` npm
  workspace.
* `apps/web/matrix-workspace` keeps its git history through the move, and its
  test suite stayed green across it (46 tests at the time of the move).
* `mm/` is untouched, per `AGENTS.md:28` and `PREREQUISITES.md:57`.
* Everything removed is recoverable from git history; nothing was deleted that
  was not either relocated or a placeholder.
