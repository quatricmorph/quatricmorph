# ADR-006 — WeightQL has no arbitrary code execution, structurally

**Status:** Accepted — **not revisitable without a new ADR**
**Date:** 2026-08-03

## Context

`mm/viz.js:119-126` builds a matrix initializer by calling `eval?.()` on a
user-supplied string:

```js
return eval?.(`(i, j, h, w) => { try { return (${expr}) } catch (e) { return 0 } }`)
```

The string reaches that call from the **query string**: `mm/index.html:531`
restores state from the URL, `mm/util.js:86-102` writes it into
`params.left.expr`, and `viz.getInitFunc` (line 132) dispatches to
`tryEvalInitExpr`. A crafted link therefore runs attacker-chosen JavaScript in a
visitor's browser. `mm/util.js:62-78` is a milder second instance: it fetches an
arbitrary URL synchronously and applies the response as application state.

For `mm` — a local research visualizer driven by hand-entered expressions —
this is a reasonable convenience. Quatricmorph is a product surface that opens
files from disk and serves an HTTP API, and the same code stops being
reasonable the moment it is in that position.

## Decision

WeightQL cannot express arbitrary computation, and this is enforced by
construction rather than by validation:

1. **`q_expression::Expr` is a closed enum.** Its variants are `TensorRef`,
   `Slice`, `Transpose`, `MatMul`, `Add`, `Sub`, `Reduce`, and `Compare`. There
   is no `Call(name, args)` variant, so there is nowhere for an unknown function
   to go. Adding one would require editing the enum, which is a visible change.
2. **The function set is a fixed list**: `tensor`, `transpose`, `compare`, and
   eight named reductions. The parser rejects any other identifier followed by
   `(` with a message naming the whole permitted set.
3. **No production for a function definition.** There is no `fn`, no `lambda`,
   no `=>`, no `Function`.
4. **Strings support exactly two escapes**, `\"` and `\\`. Anything else is a
   parse error, so a string cannot smuggle a newline or a control character.
5. **No raw SQL passthrough.** `SELECT` is a WeightQL keyword with two accepted
   targets, `value` and `slice`; it does not reach the catalog's SQL. The
   catalog's own filter interpolation uses only `&'static str` values derived
   from enums, never user input.
6. **The initializer-from-expression feature is not ported.**
   `apps/web/quatricmorph-workspace` has no `eval` path.

## Alternatives considered

**A sandboxed expression evaluator** (a small interpreter over a safe subset).
Rejected for this pass: it is a second language to specify, test, and secure,
and nothing in ARCHITECTURE.md's MVP needs it. WeightQL's job is to *address*
tensors and *describe* operations, not to compute scalar functions.

**Allowing `eval` behind a "local only" flag.** Rejected: the daemon binds a
socket, "local only" is a deployment property rather than a code property, and a
flag that disables a security boundary is a boundary that will be on in
someone's setup.

**Validating the expression string with a denylist** (`eval`, `Function`,
`require`, `import`, backticks…). Rejected: denylists on a language as
permissive as JavaScript do not hold. The closed enum is not a filter that can
be evaded — the capability is absent.

## Consequences

* WeightQL can never grow a user-defined function without editing
  `q_expression::Expr`, which is where this ADR will be cited.
* Tests assert the rejection on both sides:
  `crates/q-weightql/src/parser.rs::arbitrary_code_execution_constructs_are_rejected`
  and
  `apps/web/query-interface/src/__tests__/weightql.test.ts` ("rejects
  arbitrary-code-execution constructs"), over the same hostile corpus.
* A user who wants a computed initializer must express it in WeightQL's
  vocabulary or not at all. That is a real loss of expressiveness, accepted
  deliberately.
* `docs/CURRENT_ARCHITECTURE.md` §5 records the `mm` finding as evidence, with
  the reuse decision **Deprecate**.
