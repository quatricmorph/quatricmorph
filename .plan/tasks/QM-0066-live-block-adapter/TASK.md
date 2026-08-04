# QM-0066 — Live tensor-block adapter

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 06 — Grid matrix workspace

## Objective

Make `DaemonBlockSource` fetch real checkpoint data — while keeping its refusal
behaviour for anything unbounded.

## Repository Evidence

* `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts:107` —
  `DaemonBlockSource`, a stub. Its test is named
  `the_daemon_source_refuses_rather_than_returning_plausible_zeros` (`GRID-004`).
* `:61` `assertBlockIsBounded`; `:83` `HandEnteredSource` (works); `:39`
  `Fidelity`; `:53` the `TensorBlockSource` interface.
* `refuses_a_block_that_would_pull_a_whole_tensor_into_the_browser` (`GRID-005`
  Verified).
* Daemon `GET /v1/tensors/{id}/blocks?rows=&columns=&format=qtile` (`API-003`
  Verified — `block_route_returns_only_the_requested_window`).
* `q-tiles` `.qtile` v1 decode.

## Requirements Covered

`GRID-004`, `MVP-25`.

## Dependencies

`QM-0064`, `QM-0032`.

## Blocks

`QM-0067`, `QM-0080`.

## Parallelization

Sequential before `QM-0067`. Needs a running daemon.

## Program Boundary

`apps/web/quatricmorph-workspace/src/tensor`, plus a Web Worker.

## Scope

* `DaemonBlockSource.fetch` → bounds check → HTTP → `.qtile` decode in a **Web
  Worker** → `TensorBlockData` with fidelity and provenance.
* A TypeScript `.qtile` decoder matching the Rust encoder.
* Cancellation of an in-flight fetch.
* Fidelity carried from the tile's encoding to the badge.

## Out of Scope

Matmul on real blocks (`QM-0067`) · writing blocks · caching beyond HTTP.

## Files Expected to Change

* `apps/web/quatricmorph-workspace/src/tensor/block-adapter.ts`

## Files Expected to Add

* `apps/web/core/src/qtile/decoder.ts`
* `apps/web/quatricmorph-workspace/src/tensor/qtile-worker.ts`
* `apps/web/core/src/__tests__/qtile-decoder.test.ts`
* `apps/web/quatricmorph-workspace/e2e/block-fetch.spec.ts`

## Files Expected to Remove or Deprecate

None. **`the_daemon_source_refuses_rather_than_returning_plausible_zeros` must
keep passing** for the error path — a fetch failure must still refuse rather than
return zeros.

## Data Contracts

The TypeScript decoder must match `crates/q-tiles` exactly: magic
`QTILE\0\0\0`, 72-byte header, little-endian, three encodings at 4 / 2 / 8 bytes
per cell. **A cross-language decode test is required**, using a `.qtile` produced
by the Rust encoder as a checked-in fixture.

```ts
type TensorBlockData = {
  values: Float32Array; rows: number; columns: number
  dtype: string; fidelity: Fidelity
  provenance: { canonicalAddress: string; blockExtent: …; bytesRead: number }
}
```

## Memory and Performance Constraints

* `MAX_BLOCK_REQUEST_BYTES = 4 MiB`; checked **before** the network call.
* Decode in a Worker so a 256 KiB decode never stalls an animation frame.
* Block fetch + decode budget: < 200 ms for 256×256.
* `Float32Array` transferred, not copied, from the Worker.

## Implementation Plan

1. Write the TypeScript `.qtile` decoder in `apps/web/core`.
2. Add a Rust-produced `.qtile` fixture; assert the decoder matches expected
   values exactly.
3. Implement the Worker wrapper with transferable buffers.
4. Implement `DaemonBlockSource.fetch`: bounds check → fetch → decode → map
   fidelity from encoding.
5. Add `AbortController` cancellation.
6. Keep the refusal path for every error.

## Error Handling

* Over the transfer ceiling → refuse **before** the network, suggesting a smaller
  extent.
* HTTP error → refuse with the status; **never return zeros**.
* Decode failure → refuse naming the corruption; never partial data.
* Version mismatch → refuse naming both versions.
* Cancellation → the Worker is terminated; no partial data reaches the scene.

## Acceptance Criteria

1. A 256×256 block fetches, decodes, and renders with real values.
2. Values match `golden.json` for known indices.
3. The TypeScript decoder matches the Rust encoder **byte for byte**, on a
   checked-in fixture.
4. A whole-tensor request is refused before the network.
5. An HTTP error refuses; **no zeros are rendered**.
6. A corrupt `.qtile` refuses; no partial render.
7. Decoding happens in a Worker; the main thread is not blocked (measured).
8. Fetch + decode < 200 ms for 256×256.
9. Cancellation leaves no partial data.
10. `GRID-005`'s refusal test still passes.

## Verification Plan

**Automated** — decoder cross-language test; adapter tests for every refusal
path; Playwright fetch-and-render with a golden-value comparison.
**Manual** — fetch a block; compare a rendered cell against `q-cli value`.

## Suggested Commands

```bash
cargo run -p q-daemon -- --model-root fixtures/tiny-llama-large     # verified today
cd apps/web && npx vitest run qtile-decoder                          # introduced here
npx playwright test apps/web/quatricmorph-workspace/e2e/block-fetch.spec.ts
```

## Test Cases

| Input | Expected |
| --- | --- |
| 256×256 block request | Fetches, decodes, renders |
| Value at `[100, 42]` | Matches `golden.json` |
| Rust-encoded fixture decoded in TS | Exact match |
| 4096×4096 request | Refused before the network |
| Daemon returns 500 | Refuses; **no zeros** |
| Truncated `.qtile` | Refuses naming the corruption |
| `version: 2` tile | Refuses naming both versions |
| Main-thread blocking during decode | None (measured) |
| Cancel mid-fetch | No partial data |
| Quantized tile | Badge reads `quantized`, not `exact` |

## Risks

| Risk | Mitigation |
| --- | --- |
| Two `.qtile` decoders drift | Cross-language test against a Rust-produced fixture |
| An error path returns zeros | The existing test name is the specification; it stays |
| Worker transfer copies instead of transferring | Asserted by checking the source buffer is detached |

## Completion Evidence

* Cross-language decoder test output.
* Golden-value comparison for fetched cells.
* Main-thread blocking measurement.
* Refusal output for each error case.
* Fetch + decode timing.
