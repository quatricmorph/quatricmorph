# ADR-CANDIDATE-012 — WeightQL parser technology

## Status

`Open`.

## Context

WeightQL is parsed **twice** — once in Rust for execution, once in TypeScript for
the KaTeX preview and the error caret. Two hand-written parsers for one grammar
will drift, and drift means the preview shows one grouping while the daemon
executes another.

## Repository evidence

* `crates/q-weightql/` — `lexer.rs` (321), `parser.rs` (640), `plan.rs` (673).
  The grammar is documented in `parser.rs:7-24`.
* `apps/web/query-interface/src/weightql.ts` — the browser parser, 17 tests.
* `docs/decisions/ADR-005-hand-written-parsers.md` — already accepted.
* `docs/decisions/ADR-006-weightql-no-arbitrary-execution.md` — the closed-enum
  rationale.
* `crates/q-expression/src/lib.rs:147` — `Expr`, *"A closed set"*.
* `WQL-001`…`WQL-005`, `WQL-009`…`WQL-012` **Verified**; `CHAT-002`, `SEC-003`
  **Verified** on the browser side.

## Decision required

Keep two hand-written parsers, unify them, or generate both?

## Options

| Option | |
| --- | --- |
| **A** | Two hand-written parsers + a shared conformance corpus |
| **B** | Compile the Rust parser to WASM; the browser calls it |
| **C** | A parser generator (pest, nom, ANTLR) emitting both |
| **D** | One parser in the daemon; the browser round-trips for every keystroke |

## Advantages

* **A** — no new tooling; both are done and tested; error messages are
  hand-tuned in each language, which is why the caret works (`CHAT-004`).
* **B** — one grammar, structurally. Drift becomes impossible.
* **C** — one grammar file; both sides generated.
* **D** — one implementation, and it is the authoritative one.

## Disadvantages

* **A** — **drift is possible**, and it is silent.
* **B** — a WASM build step, a payload for a 640-line parser, and awkward error
  mapping across the FFI boundary; the caret and the "unknown function names the
  closed set" message would both degrade.
* **C** — ANTLR needs a Java toolchain; pest and nom are Rust-only, so the
  TypeScript side would still be hand-written; a generator would also be a
  regression from the tuned error messages `ADR-005` chose deliberately.
* **D** — a network round trip per keystroke makes the preview laggy and makes
  the editor useless offline.

## Risks

**[`RISK_REGISTER.md`](../RISK_REGISTER.md) R6.** The failure mode is that a
query previews as one grouping and executes as another — invisible until a result
is wrong.

## Recommended default

**A**, with a **shared conformance corpus** at
`schemas/weightql/conformance.json`, run by both suites:

```jsonc
[
  { "input": "show tensor(\"Q[10]\") @ transpose(tensor(\"K[10]\"))",
    "ast": { "MatMul": [ { "TensorRef": "Q[10]" },
                         { "Transpose": { "TensorRef": "K[10]" } } ] } },
  { "input": "A = tensor(\"Q[10]\"); show A + A",   "ast": "…" },
  { "input": "show eval(\"x\")",  "error": "unknown_function" },
  { "input": "show tensor(\"Q[10]\"",  "error": "unexpected_eof", "caret": 21 }
]
```

Every case asserts the same AST **and** the same error class in both languages.
That converts an invisible divergence into a red test, which is the property
option **B** would provide structurally and at much higher cost.

Revisit **B** only if the corpus catches drift more than twice — at that point
the maintenance argument has evidence behind it.

## Tasks affected

`QM-0071`, `QM-0072`, `QM-0074` (adds the corpus).

## Decision deadline

Before `QM-0074`.

`QM-0071` and `QM-0072` appear earlier in `Tasks affected` but **do not commit**
(`README.md` §"How a deadline is derived") — both use the two existing parsers
unchanged. `QM-0074` is the first task that must know whether the answer is a
shared corpus, a WASM build, or a generator, because it is the task that adds
`schemas/weightql/conformance.json`.
