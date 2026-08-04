# ADR-CANDIDATE-013 — Browser test strategy

## Status

`Open`.

## Context

101 web tests run in vitest today, none of which touch a renderer. The MVP adds
Cesium rendering, picking, and WebGL-backed sphere rendering — none of which
vitest can exercise, because there is no GL context in Node.

## Repository evidence

* `apps/web/vitest.config.ts`, plus per-package configs. 12 test files, 101 tests,
  832 ms.
* Existing tests are all pure: `lod-policy`, `tile-client`, `matmul`, `blocking`,
  `animation-schedule`, `grid-ruler`, `array2d`, `weightql`, `params`, `expr`,
  `interaction`, `block-adapter`.
* `MATMUL-005` — *"Demo still runs after extraction"* — is verified by
  `npx vite build` succeeding, **not** by rendering anything.
* `.github/workflows/build.yaml` `web` job — vitest + a `quatricmorph-workspace` build.
* `MVP-18`, `MVP-21`, `MVP-41`, `MVP-43` all require a real browser.

## Decision required

How are rendering, picking, and memory verified?

## Options

| Option | |
| --- | --- |
| **A** | Playwright for render/pick/memory; vitest stays for logic |
| **B** | vitest + `jsdom` + a mocked WebGL context |
| **C** | vitest + `@vitest/browser` |
| **D** | Manual only |

## Advantages

* **A** — a real browser, real WebGL, real Cesium; screenshots as evidence;
  `page.metrics()` gives the heap numbers `MVP-41` needs; runs in CI headless.
* **B** — no new tooling.
* **C** — one runner for everything.
* **D** — no infrastructure.

## Disadvantages

* **A** — a new dev dependency and a CI job; browser downloads in CI.
* **B** — **a mocked GL context tests the mock.** It cannot detect that Cesium
  failed to render, which is precisely R1's failure mode.
* **C** — `@vitest/browser` was still maturing; screenshot and metrics APIs are
  weaker than Playwright's.
* **D** — `MVP-41` and `MVP-43` need repeatable measurement, and a human counting
  console errors across 100 iterations is not that.

## Risks

* **A** — flakiness in browser tests eroding trust in the suite. Mitigation: keep
  the browser suite **small and assertive** — does it render, does a pick resolve
  correctly, does the heap return to baseline. Not a visual-regression suite.
* CI time. Mitigation: a separate job, parallel with the others.

## Recommended default

**A.**

```text
vitest      pure logic — grid math, LOD policy, parsers, matmul, blocking
            (101 tests today; every new pure module goes here)

Playwright  render/pick/memory — a small, decisive set:
            · a generated tileset loads and renders           MVP-18
            · camera approach refines LOD                     MVP-19
            · navigation triggers no exact-value request      MVP-20
            · a pick resolves to the expected canonical address MVP-21
            · 100 model switches: heap within 10% of baseline MVP-41
            · the console is empty across the checklist       MVP-43
```

Six browser tests, each mapping to an acceptance criterion. That is the smallest
set that covers what vitest structurally cannot, and small is what keeps a
browser suite trustworthy.

Screenshots are archived as CI artifacts and referenced from each task's
`Completion Evidence`.

## Tasks affected

`QM-0050`, `QM-0051`, `QM-0052`, `QM-0053`, `QM-0080`, `QM-0082`, `QM-0085`.

`QM-0050` was missing from this list. Its Repository Evidence already names this
candidate (*"`ADR-CANDIDATE-009` (local ENU frame), `010` (no framework), `013`
(Playwright)"*), and the spike's go/no-go depends on being able to observe
whether Cesium rendered anything — which is the capability under decision here.

## Decision deadline

Before **`QM-0050`**, the earliest task in `Tasks affected`.

Corrected from `QM-0051`. `QM-0050` is scheduled in Wave 1, three waves earlier,
*because* it de-risks R1 — and a spike that cannot measure its own outcome does
not de-risk anything. See `README.md` §"How a deadline is derived".
