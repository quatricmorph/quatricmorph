# API_CONTRACTS — the local daemon

## 0. State

`crates/q-daemon` is axum over tokio, 941 lines, 8 tests. **Eight routes serve
real data; five return `501` carrying a requirement ID.**

Transport decision: **HTTP for request/response, Server-Sent Events for
progress** (`ADR-CANDIDATE-011`). HTTP is already built and tested; SSE is one
route rather than a second protocol, and unlike WebSocket it needs no framing,
no heartbeat, and no reconnection logic for a local service.

---

## 1. Route inventory

| Method | Route | State | Requirement |
| --- | --- | --- | --- |
| `GET` | `/v1/models` | ✓ | `API-001` |
| `GET` | `/v1/models/{modelId}` | ✓ | `API-001` |
| `GET` | `/v1/models/{modelId}/layers` | ✓ | `API-001` |
| `GET` | `/v1/models/{modelId}/tensors` | ✓ | `API-001` |
| `GET` | `/v1/tensors/{tensorId}` | ✓ | `API-002` |
| `GET` | `/v1/tensors/{tensorId}/value` | ✓ | `API-002`, `AC-005` |
| `GET` | `/v1/tensors/{tensorId}/blocks` | ✓ | `API-003` |
| `POST` | `/v1/query` | ✓ (scalar/slice execute; matmul plans) | `API-004` |
| `GET` | `/v1/tensors/{tensorId}/statistics` | **501** | `STAT-002` → `QM-0020` |
| `GET` | `/v1/visualizations/{modelId}/tileset.json` | **501** | `CESIUM-001` → `QM-0044` |
| `GET` | `/v1/visualizations/{modelId}/tiles/{tileId}.glb` | **501** | `GLB-001` → `QM-0042` |
| `GET` | `/v1/visualizations/{modelId}/tiles/{tileId}.qtile` | **501** | `TILE-004` → `QM-0041` |
| `POST` | `/v1/conversions` | **501** | `JOB-002` → `QM-0033` |
| `GET` | `/v1/jobs/{jobId}` | new | `API-009` → `QM-0033` |
| `GET` | `/v1/jobs/{jobId}/events` | new (SSE) | `API-010` → `QM-0033` |
| `POST` | `/v1/jobs/{jobId}/cancel` | new | `API-009` → `QM-0033` |
| `POST` | `/v1/jobs/{jobId}/resume` | new | `API-009` → `QM-0033` |
| `POST` | `/v1/query/{planId}/cancel` | new | `API-011` → `QM-0073` |
| `GET` | `/v1/cache` | new | `API-012` → `QM-0032` |
| `DELETE` | `/v1/cache` | new | `API-012` → `QM-0032` |

---

## 2. Status code contract

Already implemented and tested; new routes inherit it.

| Code | Meaning | Test |
| --- | --- | --- |
| `200` | Success. Body carries `fidelity` | — |
| `202` | Job accepted; body carries `job_id` | `QM-0033` |
| `400` | Malformed request, **including shape mismatch — before any read** | `a_shape_mismatch_is_a_400_before_any_read` |
| `403` | Path outside a configured model root | `the_model_root_boundary_is_enforced` |
| `404` | No such model, tensor, tile, or job | — |
| `409` | **Ambiguous alias**; body carries candidates | `an_ambiguous_alias_is_a_409_carrying_its_candidates` |
| `413` | Request exceeds a cost threshold; body names the threshold | `QM-0073` |
| `499` | Cancelled by the client | `QM-0073` |
| `500` | Internal error; no internal paths in the body | — |
| `501` | **Declared gap.** Body carries `requirement` and an explanation | `unbuilt_routes_return_501_with_a_requirement_id` |

`501` is the load-bearing one. It is not an error: the client treats it as a
declared gap and does not retry
(`treats_a_501_as_a_declared_gap_not_a_failure_to_retry`). A capability that does
not exist says so, by ID, so the caller can look it up in `STATUS.md`.

### Error body

```jsonc
{
  "error": "not_implemented",
  "message": "Tile pyramid generation is not built.",
  "requirement": "TILE-004",          // populated for 501
  "candidates": [ /* populated for 409 */ ],
  "threshold": { /* populated for 413 */ }
}
```

---

## 3. Representative payloads

### `GET /v1/tensors/{tensorId}/value?index=100,42`

```jsonc
{
  "value": 0.006408154033124447,
  "canonical_address": "model.layers[10].self_attention.query_projection.weight",
  "index": [100, 42],
  "dtype": "F32",
  "fidelity": "exact",
  "provenance": {
    "shard": "model-00002-of-00002.safetensors",
    "byte_offset": 419928,
    "bytes_read": 4
  }
}
```

**4 bytes read.** `scalar_read_touches_only_dtype_width_bytes` holds this, and it
is the single clearest demonstration of the architecture's premise.

### `GET /v1/tensors/{tensorId}/blocks?rows=0:256&columns=0:256&format=qtile`

`Content-Type: application/octet-stream`, body is a `.qtile`. Fidelity travels in
the header (`X-Quatricmorph-Fidelity: exact`) and in the tile's own encoding
field. `format=json` returns values inline and is capped at 4 096 elements —
above that the caller must take a `.qtile`.

### `POST /v1/query`

```jsonc
// request
{ "model": "…", "expression": "show tensor(\"Q[10]\")[0:256,:] @ transpose(tensor(\"K[10]\"))",
  "mode": "plan" }        // plan | execute

// 200, mode=plan
{ "plan_id": "plan:b3:…", "status": "planned",
  "estimated_read_bytes": 524288, "estimated_gpu_bytes": 786432,
  "execution_tier": "cpu-block", "fidelity": "exact",
  "result_shape": [256, 256], "requires_confirmation": false,
  "references": [ /* resolved, with byte ranges */ ] }
```

`mode: "execute"` runs it and returns the result schema from
[`WEIGHTQL_ARCHITECTURE.md`](WEIGHTQL_ARCHITECTURE.md) §9. A plan above
`WARN_READ_BYTES` sets `requires_confirmation: true`, and `execute` without a
matching `plan_id` is rejected — so a cost the user never saw cannot be paid.

### `POST /v1/conversions`

```jsonc
// request
{ "model_id": "…", "scope": { "kind": "tensor", "canonical_address": "…" },
  "lod_range": [0, 4], "block_size": [256, 256],
  "backend": "auto", "encoding": "quantized_i16" }

// 202
{ "job_id": "job:…", "state": "Pending", "events": "/v1/jobs/job:…/events" }
```

`scope.kind` ∈ `model | subsystem | layer | tensor | block`. Converting a whole
trillion-parameter model is legal to *request* and will take as long as it takes;
it is checkpointed, cancellable, and resumable, and the job record reports
progress. What it never does is allocate proportionally to the model.

### `GET /v1/jobs/{jobId}/events` — SSE

```text
event: progress
data: {"job_id":"job:…","state":"Converting","phase":"blocks",
       "current_tensor":"model.layers[10]…","blocks_done":412,"blocks_total":1024,
       "bytes_read":107374182,"bytes_written":5242880,"elapsed_ms":18300}

event: complete
data: {"job_id":"job:…","state":"Complete","tileset_uri":"/v1/visualizations/…/tileset.json"}
```

---

## 4. Job model

`JOB-001` and `JOB-003` are verified: the state machine rejects illegal
transitions, and jobs persist and reload with their state.

```text
Pending → Inspecting → Indexing → Converting → Writing → Validating → Complete
                                       ↓
                                    Paused ⇄ Converting
                                       ↓
                          Cancelled / Failed  →  (resumable)
```

`failed_and_cancelled_jobs_can_resume` is already tested. The job record carries
job ID, source model ID, conversion version, configuration hash, current phase,
current tensor, current block, completed and failed block lists, bytes read and
written, GPU and CPU time, cache hits, errors, and start/update timestamps —
exactly the task specification §23 field set.

**Checkpoint granularity is one block**, so a crash costs at most one block of
work.

---

## 5. Security

| Control | Rule | State |
| --- | --- | --- |
| Bind address | `127.0.0.1` only. Never `0.0.0.0` without an explicit flag | `QM-0075` |
| Model roots | Every path resolved and confined to a configured root | ✓ `SEC-001` |
| Path traversal | `../` refused after canonicalization; symlinks resolved | ✓ |
| CORS | Explicit allowlist of the local dev origins; no `*` | New — `SEC-007` |
| Static files | Served only from the generated-artifact directory, never from a user-supplied path | `QM-0044` |
| Request size | Bounded body; bounded query string | New |
| Concurrency | Bounded in-flight requests and one job executor; excess queued, not spawned | `QM-0033` |
| Cost limits | §5.1 of the WeightQL doc, enforced server-side, never client-side only | `QM-0073` |
| Error bodies | Never contain absolute paths or internal state | Audit in `QM-0085` |

Client-side cost checks are a courtesy. The server enforces, because the client
is not a trust boundary even when it is ours.

---

## 6. Versioning

`/v1/` prefix. Within a major version, only additive changes: new routes, new
optional request fields, new response fields. Removing a field, changing a type,
or changing a status code's meaning is a `/v2/`. The 501 routes are **not** a
breaking change when they start returning `200` — the client already handles
both.
