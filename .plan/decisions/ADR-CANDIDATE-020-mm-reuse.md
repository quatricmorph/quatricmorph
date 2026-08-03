# ADR-CANDIDATE-020 — `mm` reuse versus extraction

## Status

`Decided` — recording an executed decision.

## Context

The task specification asks whether existing `mm` code is reused, extracted, or
replaced. The work is done; this records what was decided and why, so that
nothing is re-litigated.

## Repository evidence

* `mm/` — 5 216 lines: `viz.js` (2 105), `index.html` (839), `gui.js` (380),
  `util.js` (365), plus `ref.html` (1 527), vendored libraries, assets, an intro
  article, and `LICENSE` (MIT, Meta Platforms, Inc.).
* `docs/CURRENT_ARCHITECTURE.md` — 305 lines, a **per-symbol** decision record
  with a tally: 4 reuse-as-is, ~45 extract, ~20 extract-and-refactor, 9 deprecate.
* `apps/web/matrix-workspace/src/` — 40 modules across `math/`, `layout/`, `viz/`,
  `interaction/`, `app/`, `gui/`, `tensor/`, `util/`; 74 tests.
* `apps/web/matrix-workspace/{LICENSE,NOTICE.md}` — MIT text reproduced with
  attribution.
* `AGENTS.md` — *"`mm/` Historical matrix-viz reference — read-only; do not
  delete; not product surface."*
* `docs/decisions/ADR-002-crates-rewritten-not-migrated.md`.

## Decision required

None. Recording.

## What was decided, per category

| Decision | Count | Examples |
| --- | --- | --- |
| **Reuse as-is** | 4 | `ball.png`, the Droid Sans typeface, `intro/`, `LICENSE` |
| **Extract** | ~45 | `Array2D`, `Mat`, epilogs, initializers, defaults, layout constants, geometry helpers, examples |
| **Extract and refactor** | ~20 | `MatMul` (math split from scene), `grid` → `math/blocking.ts`, animation cursors → `math/animation-schedule.ts`, the `params` bag → domain modules, `initGui` → two GUIs, `disposeAndClear` → dispose materials too |
| **Deprecate** | 9 | `tryEvalInitExpr`, `tryLoadData`/`tryURLInit`, the `config`-URL branch, `sampleSphere`, the import map, five vendored libraries |

## Why extraction rather than a rewrite

`mm`'s **math is correct and proven**. `dotprod`, `ikjmul`, the `grid` block
iteration with its dead-final-block guard, and the three animation cursors encode
behaviour that took real effort to get right. Rewriting them would have
discarded working code to obtain the same result with new bugs.

`mm`'s **structure was the problem**: data, statistics, and presentation lived in
one `Mat` object; a single untyped `params` bag was reached into by every
subsystem; any parameter change rebuilt the whole scene. Extraction separated
those without touching the arithmetic — which is why `math/` has 34 tests and no
Three.js import.

## Why nine deprecations

The load-bearing one is `tryEvalInitExpr`: `eval` reachable from a URL parameter
(`mm/viz.js:119-126` ← `mm/index.html:531` ← `mm/util.js:86-102`). A crafted link
executes attacker-chosen JavaScript in the visitor's browser.

`docs/CURRENT_ARCHITECTURE.md` §5 is careful, and correct, about the framing:
this is not a criticism of `mm`, which is a research visualizer meant to be run
locally with hand-entered expressions. *"It stops being reasonable the moment the
same code is served as a product surface."*

That single finding is the origin of the closed-expression design in
`q-expression` and of `ADR-006`.

## Defects found and fixed during extraction

1. `viz.js:357-363` — `Array2D.map` references an undefined `n`; would throw.
   **Fixed** in `viz/array2d.ts`.
2. `viz.js:60-64` — `sampleSphere` references an undefined `sm`. **Deprecated**.
3. `util.js:184,186` — arrow colour alpha assigned twice in one statement; the
   third vertex's alpha is never set. **Recorded**.
4. `util.js:343-347` — `disposeAndClear` disposes geometries but not materials or
   textures. **Assigned to `QM-0067`**.
5. `viz.js:186` — the author's own *"TODO the way epis are done is kind of messy"*.
6. `index.html` — `window` listeners added and never removed. **Assigned to
   `QM-0056`**.

## Licensing

`mm/LICENSE` unmodified; the MIT text reproduced at
`apps/web/matrix-workspace/LICENSE` with attribution in `NOTICE.md`;
`package.json` names `mm` in its description. `mm/` is read-only and no task in
this plan modifies it. Audited by `QM-0093`.

## Tasks affected

`QM-0065`, `QM-0067` (defects 4 and 6), `QM-0093` (attribution audit).

## Decision deadline

Passed.
