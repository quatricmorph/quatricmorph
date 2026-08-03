# ADR-005 — Hand-written recursive-descent parsers, no parser generator

**Status:** Accepted
**Date:** 2026-08-03
**Applies to:** `q-nsir` (canonical addresses, aliases), `q-weightql`
(tokenizer, parser), `apps/web/query-interface` (the client-side mirror)

## Context

Four grammars needed parsing in this pass:

1. canonical addresses — `model.layers[10].self_attention.query_projection.weight[100,42]`
2. contextual aliases — `Q[10][100,42]`, `MLP.down[24][:]`, `Expert[12,37].up[0:128,:]`
3. WeightQL — assignments, `show`, `SELECT value ... AT`, `SELECT slice ... ROWS ... COLUMNS`
4. the same WeightQL, again, in TypeScript for the query interface

All four are small: no left recursion, no ambiguity requiring lookahead beyond
one token, no operator table deeper than three precedence levels.

## Decision

Hand-written recursive descent, sharing one character cursor between the address
and alias grammars (`q_nsir::address::Cursor`), and a separate
tokenizer + parser pair for WeightQL.

## Alternatives considered

**`nom` (parser combinators).** Genuinely appealing for grammars this size, and
the combinator style reads well. Rejected on error quality: WeightQL errors are
shown to a person typing an expression, and a combinator failure reports "at
this position, one of N alternatives failed" rather than "expected `]`". Getting
`nom` to produce the message a hand-written parser produces for free means
wrapping most productions in `context()`, at which point the combinator saving
is gone.

**`pest` (PEG from a grammar file).** Rejected: the grammar would live in a
`.pest` file separate from the AST, which is one more thing to keep in sync, and
error positions come back as spans into a parse tree rather than as the
"expected X, found Y" form the CLI and the query interface both print.

**`lalrpop` (LALR generator).** Rejected: a build-script code generator for four
grammars this small is disproportionate, and LALR conflict messages are
notoriously hard to act on.

**Share the parser between Rust and TypeScript via WebAssembly.** Rejected for
this pass: it would make the query interface depend on a wasm build of
`q-weightql`, which is a build-system commitment out of proportion to a text box
that needs to say "expected `]`". The duplication is instead pinned by keeping
the token set, the function list, and the rejection behaviour identical, with
tests on both sides asserting the same accept/reject decisions.

## Consequences

* Error messages name the expected token and the byte offset:
  `expected `]` at byte 14, found end of input`. The query interface renders a
  caret under that offset.
* The grammar is the code. There is no `.pest` or `.lalrpop` file to drift from
  the AST, and `docs`/`schemas/weightql/schema.json` document it in EBNF for
  readers.
* Adding a production means editing two parsers (Rust and TypeScript). That is
  the real cost of this decision, and it is why the TypeScript side's tests
  mirror the Rust side's case for case.
* No new dependencies: `q-nsir` and `q-weightql` parse with `std` only.
