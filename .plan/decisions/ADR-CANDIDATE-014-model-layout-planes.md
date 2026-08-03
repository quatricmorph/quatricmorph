# ADR-CANDIDATE-014 — Model layout algorithm and plane mapping

## Status

`Open`. **This candidate resolves a verified contradiction between
`ARCHITECTURE.md` and the code.**

## Context

Two questions. Which operand goes on which plane? And how is the model laid out
in the viewer's scene?

## Repository evidence — the contradiction

`ARCHITECTURE.md` §8.2:

```text
A: XY plane      B: YZ plane      C: XZ plane
```

`apps/web/matrix-workspace/src/layout/grid-ruler.ts:9-10`, in the module doc, and
implemented in `placeOperands`:

```text
World X → J (output cols), Y → I (output rows), Z → K (contraction)
A on I×K,  B on K×J,  C on I×J
```

Resolving the code's own mapping:

| Operand | Spans | Axes | Plane |
| --- | --- | --- | --- |
| A | I × K | Y × Z | **YZ** |
| B | K × J | Z × X | **XZ** |
| C | I × J | Y × X | **XY** |

So the code says A:YZ, B:XZ, C:XY. `ARCHITECTURE.md` §8.2 says A:XY, B:YZ,
C:XZ. **A and C are exchanged.**

The task specification §16 independently states `World X → J, Y → I, Z → K` with
`A → I×K, B → K×J, C → I×J` — agreeing with the code.

Supporting evidence for the code: 13 passing tests in
`layout/__tests__/grid-ruler.test.ts`, including
`every_operand_placement_it_produces_is_on_grid`; and `placeOperands` preserves
`mm`'s proven polarity, left/right, and result-placement semantics.

## Decision required

1. Which mapping is authoritative?
2. What is the model-scale layout rule in the viewer?

## Options — mapping

| Option | |
| --- | --- |
| **A** | Keep the code's mapping; correct `ARCHITECTURE.md` §8.2 |
| **B** | Change the code to match §8.2 |
| **C** | Leave both, document the divergence |

## Advantages

* **A** — two independent sources (the code, the task specification) agree; 13
  tests keep passing; `mm`'s proven placement survives.
* **B** — the SoT document stays literally correct without an edit.
* **C** — no work.

## Disadvantages

* **A** — edits `ARCHITECTURE.md`, which this plan otherwise treats as immutable.
* **B** — **invalidates 13 tests and the `mm` placement semantics for no
  functional gain.** The two mappings are equally valid geometrically; only one is
  implemented.
* **C** — `AGENTS.md` instructs agents to follow `ARCHITECTURE.md` and *"fix or
  remove the conflicting text."* An agent reading §8.2 would implement **B**
  without knowing it was breaking anything.

## Risks

**C is the dangerous option.** Leaving the contradiction means the next agent
resolves it by accident, in whichever direction it happened to read first.

## Recommended default

**A.** Keep the code; correct `ARCHITECTURE.md` §8.2 in `QM-0090`, with a note
that the change is a documentation fix recorded by this ADR, not a design change.

## Model-scale layout

Deterministic, derived from logical addresses — never scattered offsets
([`GRID_ARCHITECTURE.md`](../GRID_ARCHITECTURE.md) §7):

```text
layer_index          → primary model axis (Z), spaced by layerSpacing
module role          → secondary grouping axis (X), in a fixed role order
tensor index in role → local tensor grid (X, Y) within the module cell
block coordinates    → local block grid within the tensor frame
scalar coordinates   → procedural cell coordinates within the block
```

Every level applies the same rule one scale down, drawing padding from the same
parameter set. Two consequences: zoom is continuous in meaning, and
`tensor_anchor` is a pure function of the canonical address — computable in the
browser with no round trip, which is what makes "fit selection" and "search by
address" instant.

## Tasks affected

`QM-0002` (registers the divergence), `QM-0060`, `QM-0062`, `QM-0090` (corrects
the document).

## Decision deadline

Before `QM-0060`.
