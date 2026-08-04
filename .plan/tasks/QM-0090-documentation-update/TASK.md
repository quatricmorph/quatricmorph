# QM-0090 — Documentation update and divergence resolution

## Status

Blocked

Unblocks when `QM-0080` reaches `Complete`.

**The ADR gate is satisfied.** `ADR-CANDIDATE-014` was promoted on 2026-08-04 to
`docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md`, which accepts
the code's mapping and assigns the §8.2 correction to this task.

## Phase

Phase 09 — Documentation and release

## Objective

Make the repository's documents describe what the repository does — including
correcting `ARCHITECTURE.md` §8.2.

## Repository Evidence

* `ARCHITECTURE.md` §8.2: `A: XY plane, B: YZ plane, C: XZ plane`.
* `apps/web/matrix-workspace/src/layout/grid-ruler.ts:9-10`, implemented and
  covered by 13 tests: `X→J, Y→I, Z→K` with `A on I×K, B on K×J, C on I×J`,
  which resolves to **A: YZ, B: XZ, C: XY**.
* The task specification §16 independently states the code's mapping.
* `AGENTS.md`: *"If any document conflicts with `ARCHITECTURE.md`, follow
  `ARCHITECTURE.md` and fix or remove the conflicting text."*
* `README.md` §"What works today" and §"What does not work yet" both become
  substantially wrong once Phases 03–07 land.

## Requirements Covered

`DOC-001`, `DOC-005`, `MVP-01`, `MVP-45`.

## Dependencies

`QM-0080`. (`ADR-CANDIDATE-014` → `ADR-009`: satisfied 2026-08-04.)

## Blocks

`QM-0091`, `QM-0094`.

## Parallelization

Parallel with `QM-0092`, `QM-0093`.

## Program Boundary

Documentation only. **This is the one task authorized to edit
`ARCHITECTURE.md`**, and only §8.2, and only after the ADR exists.

## Scope

* Correct `ARCHITECTURE.md` §8.2 with a note citing the ADR.
* Rewrite `README.md`'s "what works" and "what does not work" sections.
* Update `docs/ROADMAP.md` to reflect completed phases.
* Update `AGENTS.md`'s codebase table.
* Update `.plan/DIVERGENCE_REGISTER.md` rows to `Resolved`.
* Add a §16 note pointing at `ADR-001` for the repository-root departure.

## Out of Scope

`STATUS.md` (`QM-0091`) · limitations (`QM-0092`) · licensing (`QM-0093`) ·
any other `ARCHITECTURE.md` section.

## Files Expected to Change

* `ARCHITECTURE.md` — **§8.2 only**, plus a §16 pointer note
* `README.md`
* `docs/ROADMAP.md`
* `AGENTS.md`
* `.plan/DIVERGENCE_REGISTER.md`

## Files Expected to Add

None.

## Files Expected to Remove or Deprecate

None.

## Data Contracts

The §8.2 correction:

```text
## 8.2 3D Representation

Each operand is a plane:

    A: YZ plane   (I × K)
    B: XZ plane   (K × J)
    C: XY plane   (I × J)

World axes: X → J (output columns), Y → I (output rows), Z → K (contraction).

Note: an earlier revision of this section assigned A to XY and C to XZ. The
mapping above is what `apps/web/core/spatial/grid.ts` implements and what
`schemas/visualization/spatial-contract.json` records. See
docs/decisions/ADR-0NN-model-layout-planes.md.
```

Keeping the note matters: a reader who remembers the old text should learn it
changed and why, rather than doubting their memory.

## Memory and Performance Constraints

None.

## Implementation Plan

1. Confirm `docs/decisions/ADR-009-world-axis-binding-and-operand-planes.md`
   exists and accepts the code's mapping. (Done 2026-08-04.)
2. Apply the §8.2 correction with the note.
3. ~~Add the §16 pointer to `ADR-001`.~~ **Already applied on 2026-08-04**,
   alongside pointers at §2.1 and §5 to `ADR-003` and a second §16 pointer to
   `ADR-007`. Verify they are present and accurate; do not duplicate them.
4. Rewrite `README.md`'s two status sections from the real state.
5. Mark completed roadmap phases.
6. Update `AGENTS.md`'s table with `apps/web/core`.
7. Update the divergence register.

## Error Handling

* The ADR not yet promoted → the task stays `Blocked`. **The §8.2 edit must not
  happen without a recorded rationale.**
* A claim in `README.md` not backed by a passing test → removed, not softened.
* A divergence still open → its register row stays `Open`; only genuinely
  resolved rows change.

## Acceptance Criteria

1. `ARCHITECTURE.md` §8.2 matches the implementation and cites `ADR-009`.
2. **§8.2 is the only `ARCHITECTURE.md` section this task modifies** — verified
   by diff. The departure pointers at §2.1, §5, and §16 were added on 2026-08-04
   and are part of this task's baseline, not its diff.
3. `README.md` accurately lists what works, with commands that run.
4. `README.md`'s "does not work" section lists every remaining `Stub` and
   `Not Started` by requirement ID.
5. The roadmap reflects completed phases.
6. `AGENTS.md` lists `apps/web/core`.
7. Every divergence register row is `Resolved` or `Open` with a reason.
8. Every command in `README.md` is executed and produces the documented output.

## Verification Plan

**Automated** — a documentation-command test running every `README.md` command.
**Manual** — `git diff ARCHITECTURE.md` reviewed line by line.

## Suggested Commands

```bash
git diff ARCHITECTURE.md                      # must show §8.2 and the §16 note only
bash -n README.md                              # extract and run documented commands
cargo test --test doc_commands                 # introduced here
```

## Test Cases

| Input | Expected |
| --- | --- |
| `git diff ARCHITECTURE.md` | §8.2 only |
| §8.2 text vs `grid.ts` | Agree |
| Every `README.md` command | Runs, produces documented output |
| `README.md` "does not work" | Every remaining gap listed by ID |
| Divergence register | No row left ambiguous |
| ADR not promoted | Task refuses to proceed |

## Risks

| Risk | Mitigation |
| --- | --- |
| Editing the SoT sets a precedent | One section, one ADR, one task, one diff review |
| `README.md` overstates what shipped | Every command is executed by a test |
| The correction is made silently | The note in §8.2 records that it changed and why |

## Completion Evidence

* `git diff ARCHITECTURE.md`, reviewed.
* The promoted ADR path.
* Output of every `README.md` command.
* The updated divergence register.
