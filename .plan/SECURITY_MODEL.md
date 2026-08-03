# SECURITY_MODEL

## 1. Threat model

Quatricmorph is **local-first**: a daemon on `127.0.0.1` and a browser on the
same machine. There are no accounts, no multi-tenancy, and no remote control
plane ([`PRODUCT_SCOPE.md`](PRODUCT_SCOPE.md) §4). That removes most of the usual
surface and leaves four real threats:

| # | Threat | Why it is real here |
| --- | --- | --- |
| T1 | **A malicious link** executes code in the user's browser | This has already happened once in this codebase's lineage: `mm` reaches `eval` from a URL parameter |
| T2 | **A malicious or malformed checkpoint** crashes, hangs, or escapes | Checkpoints are downloaded from the internet by definition |
| T3 | **A malicious page** in another tab drives the local daemon | Any page can issue requests to `127.0.0.1` |
| T4 | **A user request** exhausts memory, disk, or GPU | Not adversarial — the models are genuinely terabytes |

Explicitly **not** in the model: a hostile local user (they own the machine), a
compromised OS, or supply-chain attacks on `three`/`cesium`/`rusqlite` beyond
normal dependency hygiene.

---

## 2. T1 — no arbitrary code execution

### The inherited vulnerability

`mm/viz.js:119-126`:

```js
function tryEvalInitExpr(expr) {
  return eval?.(`(i, j, h, w) => { try { return (${expr}) } catch (e) { return 0 } }`)
}
```

Reachable from a URL: `mm/index.html:531` restores state from the query string →
`mm/util.js:86-102` sets `params.left.expr` → `viz.getInitFunc` (line 132)
dispatches on `init == 'expr'` → `eval`. **A crafted link runs attacker-chosen
JavaScript in the visitor's browser.**

A second, milder instance: `updateObjectFromSearchParams`'s `config` branch
(`mm/util.js:62-78`) fetches an arbitrary URL with a synchronous
`XMLHttpRequest` and applies the response as application state.

This is not a criticism of `mm`, which is a research visualizer meant to be run
locally with hand-entered expressions. It stops being reasonable the moment the
same code is served as a product surface.

### The structural answer

| Control | Mechanism | Evidence |
| --- | --- | --- |
| Neither path is carried forward | Absence in `apps/web/` | `SEC-004` |
| Expressions are a **closed enum** | `q_expression::Expr` — no `eval`, no user-defined functions, no shell interpolation, no raw SQL | `ADR-006` |
| Rust parser rejects execution constructs | `q-weightql` | `arbitrary_code_execution_constructs_are_rejected` |
| Unknown functions named against a closed set | Error lists the legal set | `unknown_function_error_names_the_closed_function_set` |
| Browser parser matches | `query-interface/src/weightql.ts` | `rejects_arbitrary_code_execution_constructs` |

**Closed by construction, not by filtering.** Adding a dangerous capability would
require adding an enum variant, which is a code review, not an input that slips
past a regular expression.

### URL state

The `mm` URL machinery is reused (`util/params.ts`, `app/url.ts`) **without** the
`config` branch. Restored state is validated against the parameter schema and
`castToType`d against existing defaults; an unknown key is dropped, not applied.

---

## 3. T2 — malformed input

### SafeTensors

| Attack | Defence | Evidence |
| --- | --- | --- |
| Header length of `2^63` → allocation bomb | Refused **before allocating** | `absurd_header_length_is_refused_before_allocating` |
| Corrupt JSON header | Refused with context | `corrupt_json_is_rejected_with_context` |
| Offset past end of file | Refused | `range_past_end_of_file_is_rejected` |
| Offset overrunning the tensor's own extent | Refused | `run_that_overruns_the_tensor_is_rejected` |
| Duplicate tensor names, within or across shards | Refused | `duplicate_tensor_name_is_rejected`, `duplicate_tensor_across_shards_is_rejected` |
| Unknown dtype | Refused, **never guessed** | `unknown_dtype_is_rejected_not_guessed`, `fp8_refuses_rather_than_approximates` |
| Index naming a missing shard | Reported | `missing_shard_named_by_the_index_is_reported` |

### `.qtile`

Eight distinct corruptions rejected (`corrupt_and_hostile_files_are_rejected`),
including a payload claim above `MAX_QTILE_PAYLOAD_BYTES = 256 MiB` — *"a
`.qtile` is a tile, not a checkpoint; anything this large is corrupt or hostile."*

### GLB and `tileset.json`

Consumed by CesiumJS in the browser, so the browser's own parser is the first
line. Ours adds: version checked before load; a corrupt tile fails **that tile**
and leaves its siblings rendering; a tileset whose `asset.version` we do not
support is refused rather than partially interpreted; external validation in CI
(`QM-0046`) catches malformed output before a user ever loads it.

### The principle

Every one of these **refuses rather than reinterprets**. A parser that guesses
produces a plausible wrong answer, and a plausible wrong answer about a model's
weights is worse than an error, because nothing downstream can detect it.

---

## 4. T3 — local daemon boundary

| Control | Rule | State |
| --- | --- | --- |
| Bind address | `127.0.0.1` only; `0.0.0.0` requires an explicit flag **and** a warning | `QM-0075` |
| CORS | Explicit allowlist of local dev origins. **Never `*`** | New — `SEC-007` |
| Path confinement | Every path canonicalized and confined to a configured model root | ✓ `SEC-001` |
| Path traversal | `../` refused after canonicalization; symlinks resolved before the check | ✓ `path_traversal_is_refused`, `a_traversal_attempt_never_escapes_a_root` |
| Static files | Served only from the generated-artifact directory | `QM-0044` |
| SQL injection | Every caller value is a bound parameter; only enum-derived `&'static str` is interpolated | ✓ `SEC-005` |
| Error bodies | No absolute paths, no internal state | Audit in `QM-0085` |

**Why CORS matters on localhost.** Any page the user visits can issue requests to
`127.0.0.1`. Without an origin policy, a hostile page could enumerate the user's
models and read their weights. An allowlist is the entire defence, and `*` is
equivalent to no defence.

---

## 5. T4 — resource limits

Ceilings are enumerated in [`MEMORY_BUDGET.md`](MEMORY_BUDGET.md). The security
property is that **they are enforced server-side**.

| Limit | Value | Enforced |
| --- | --- | --- |
| Whole-tensor read | **Always refused** | ✓ `WQL-011` |
| Query read bytes | `MAX_READ_BYTES` = 4 GiB → `413` | `QM-0073` |
| Query warning | `WARN_READ_BYTES` = 64 MiB → confirmation required | `QM-0073` |
| Block request from the browser | 4 MiB | ✓ `GRID-005` |
| JSON result elements | 4 096; above that, a `.qtile` | `QM-0073` |
| GLB instances per tile | 262 144 | ✓ `GLB-002` |
| `.qtile` payload | 256 MiB | ✓ `TILE-006` |
| Header length | 64 MiB | ✓ `SRC-013` |
| Concurrent requests | Bounded; excess queued, never spawned | `QM-0033` |
| Concurrent jobs | One executor; others queued | `QM-0033` |
| Request body / query string | Bounded | `QM-0075` |

Client-side checks are a courtesy that keeps the UI responsive. **The client is
not a trust boundary even when it is ours** — it can be modified, replayed, or
bypassed by a direct `curl`.

---

## 6. Browser content sanitization

| Surface | Control |
| --- | --- |
| Tensor names, addresses, aliases | Rendered as **text nodes**, never `innerHTML`. A tensor name is attacker-influenced whenever the checkpoint is |
| Error messages | Text nodes |
| Model metadata from `config.json` | Text nodes; never evaluated |
| KaTeX | §7 below |
| URL state | Validated against the schema, `castToType`d, unknown keys dropped |
| CSP | `default-src 'self'`; no `unsafe-eval`, no `unsafe-inline` for scripts; fonts and workers local |

`unsafe-eval` is worth calling out: the CSP alone would have blocked `mm`'s
`eval` path even if the code had survived. Defence in depth, since the
architectural fix and the policy fix are independent.

---

## 7. KaTeX

`SEC-006`, task `QM-0075`.

| Control | Setting | Why |
| --- | --- | --- |
| Source | **Generated from the validated AST, never the user's raw string** | A string the parser rejected never reaches KaTeX at all — stronger than escaping |
| `trust` | `false` | Disables `\href`, `\url`, `\includegraphics` |
| `strict` | `"error"` | No silent fallback that could change what is displayed |
| `maxSize` / `maxExpand` | Bounded | Macro-expansion denial of service is a real KaTeX attack class |
| `throwOnError` | `true`, caught and shown | An error message beats a mis-rendered formula |
| Output | Into a container; no `innerHTML` assembly of user text | |
| Fonts | Served locally | No third-party origin, and it works offline |

---

## 8. Supply chain

| Dependency | Note |
| --- | --- |
| `rusqlite` | `bundled` — no system SQLite, so no version skew |
| `three`, `lil-gui` | npm, pinned by lockfile. Replaced `mm`'s five vendored copies, which had no version and no integrity check |
| `cesium` | Not yet added. Large; `ADR-CANDIDATE-010` |
| `katex` | Configured per §7 |
| CUDA | Feature-gated off by default; a machine without it builds normally |
| Fixtures | **Checked in.** No test touches the network — CI asserts this and regenerates fixtures to prove reproducibility |

---

## 9. What is deliberately absent

| Absent | Why |
| --- | --- |
| Authentication | Local-first, single user. Adding tokens would imply a security property the architecture does not have |
| Encryption at rest | The checkpoint is already on the user's disk unencrypted |
| Audit logging | No multi-user model to audit |
| Rate limiting by identity | No identities. Limits are per-resource instead |
| Sandboxing the daemon | It reads files the user already owns |

If Quatricmorph ever becomes multi-user or remote — `ARCHITECTURE.md` Phase 6 —
every one of these becomes mandatory, and none of them can be retrofitted
cheaply. That is a reason to keep the local-first boundary sharp rather than
letting it blur.

---

## 10. Requirements

| ID | Requirement | State | Task |
| --- | --- | --- | --- |
| `SEC-001` | File access confined to model roots | ✓ Verified | verify only |
| `SEC-002` | No arbitrary code execution (Rust) | ✓ Verified | verify only |
| `SEC-003` | No arbitrary code execution (browser) | ✓ Verified | verify only |
| `SEC-004` | `mm`'s `eval` path not carried forward | ✓ Verified | verify only |
| `SEC-005` | No SQL injection surface | ✓ Verified | verify only |
| `SEC-006` | KaTeX sanitization contract | New | `QM-0075` |
| `SEC-007` | Daemon origin policy, bind address, request limits | New | `QM-0075` |
| `SEC-008` | CSP for both web applications | New | `QM-0050` |
| `SEC-009` | Security audit at release | New | `QM-0094` |
