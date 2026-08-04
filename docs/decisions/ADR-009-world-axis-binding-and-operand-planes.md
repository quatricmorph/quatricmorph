# ADR-009 — World-axis binding and operand plane mapping

**Status:** Accepted
**Date:** 2026-08-04
**Departs from:** ARCHITECTURE.md §8.2
**Promoted from:** `.plan/decisions/ADR-CANDIDATE-014-model-layout-planes.md`

## Context

ARCHITECTURE.md §8.2 and the implementation disagree about which operand of a
matrix multiplication occupies which plane, and the disagreement has been sitting
in the repository unresolved.

ARCHITECTURE.md §8.2:

```text
A: XY plane      B: YZ plane      C: XZ plane
```

`apps/web/matrix-workspace/src/layout/grid-ruler.ts:9-10`, in the module
documentation and implemented in `placeOperands`:

```text
World X → J (output cols), Y → I (output rows), Z → K (contraction)
A on I×K,  B on K×J,  C on I×J
```

Resolving the code's mapping to planes:

| Operand | Spans | Axes | Plane |
| --- | --- | --- | --- |
| A | I × K | Y × Z | **YZ** |
| B | K × J | Z × X | **XZ** |
| C | I × J | Y × X | **XY** |

So the code says A:YZ, B:XZ, C:XY where §8.2 says A:XY, B:YZ, C:XZ. **A and C
are exchanged.** The two mappings are geometrically equivalent — either is a
consistent right-handed assignment — but only one is implemented, and a document
that names the other is an instruction to break it.

## Decision

**The code's mapping is authoritative.** World axes bind `X → J`, `Y → I`,
`Z → K`; operands place as `A on I×K`, `B on K×J`, `C on I×J`.

ARCHITECTURE.md §8.2 is corrected to match, by `QM-0090`, with a note recording
that the change is a documentation fix made under this ADR and not a design
change.

## Alternatives considered

**Change the code to match §8.2.** The source-of-truth document would stay
literally correct with no edit. Rejected: it invalidates the 13 passing tests in
`layout/__tests__/grid-ruler.test.ts` — including
`every_operand_placement_it_produces_is_on_grid` — and discards the polarity,
left/right, and result-placement semantics carried over from `mm`, which are
proven. All of that cost buys a relabelling, because the two mappings are
equally valid geometrically. Rewriting working, tested code to satisfy a
document rather than a measurement is the more likely way to lose.

**Leave both, document the divergence.** Rejected, and it is the dangerous
option. `AGENTS.md` instructs agents to follow ARCHITECTURE.md and *"fix or
remove the conflicting text"*. An agent reading §8.2 without opening
`docs/decisions/` would implement the alternative above without knowing it was
breaking anything — resolving the contradiction by accident, in whichever
direction it happened to read first.

## Why the code wins on evidence

Two independent sources agree with it, and neither is the code itself:

* the task specification §16 states `World X → J, Y → I, Z → K` with
  `A → I×K, B → K×J, C → I×J`;
* `mm`'s placement semantics, which the port preserved deliberately.

§8.2 is the only source for the other mapping.

## Model-scale layout

The same binding extends upward. Layout is deterministic and derived from
logical addresses, never from scattered offsets (`.plan/GRID_ARCHITECTURE.md`
§7):

```text
layer_index          → primary model axis (Z), spaced by layerSpacing
module role          → secondary grouping axis (X), in a fixed role order
tensor index in role → local tensor grid (X, Y) within the module cell
block coordinates    → local block grid within the tensor frame
scalar coordinates   → procedural cell coordinates within the block
```

Every level applies the same rule one scale down, drawing padding from the same
parameter set. Two consequences follow: zoom is continuous in meaning, and
`tensor_anchor` is a pure function of the canonical address — computable in the
browser with no round trip, which is what makes "fit selection" and "search by
address" instant rather than a request.

## Consequences

* `schemas/visualization/spatial-contract.json` records
  `axis_binding.world_axes = { X: J, Y: I, Z: K }` (`QM-0004`), and
  `golden-spatial.json` asserts it from both languages at gate G1 (`QM-0005`).
  Those values are now backed by a decision rather than by a recommendation.
* ARCHITECTURE.md §8.2 becomes the **only** section of that document this pass
  edits. `QM-0090` owns the edit and its acceptance criteria pin the diff.
* A future change to the binding is a change to the golden vector, which fails
  loudly in Rust and TypeScript simultaneously. That is the intended cost.
* `grid-ruler.ts`, its 13 tests, and the `mm` placement semantics are unchanged.
