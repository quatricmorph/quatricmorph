# QUERY_UI_ARCHITECTURE — chat, selectors, KaTeX

## 0. State

`apps/web/query-interface/` is four modules with 17 tests:

| Module | Requirement | State |
| --- | --- | --- |
| `weightql.ts` | `CHAT-002` — client-side parser matching the Rust grammar | ✓ Verified, 17 tests |
| `katex-preview.ts` | `CHAT-003` — renders the AST's grouping, **not the source order** | ✓ Verified |
| `app.ts` | `CHAT-004` — error caret under the offending character | ✓ Verified |
| — | `CHAT-001` — the chat assistant | **Not started, deliberately** |

The parser and preview work. What is missing is the layer that turns a
natural-language request into a plan, shows its cost, and lets the user execute
or cancel it.

---

## 1. The boundary that defines this subsystem

> `ARCHITECTURE.md` §15: *"Chat must not read weight bytes directly. It calls the
> WeightQL planner instead."*
> §19: *"Do not let chat freely execute terabyte-scale expressions."*

```text
user text
  → intent classification            (natural language | WeightQL | selector | expression)
  → WeightQL construction             ← the ONLY thing chat produces
  → POST /v1/query  (plan only)
  → candidates? → user disambiguates
  → cost preview
  → EXPLICIT execute                  ← a separate act, never implied
  → result, labelled by fidelity
```

Chat has **no file access, no byte access, and no execution authority**. It is a
query author. Every capability it appears to have is the daemon's, exercised
through a validated plan.

This is not merely a safety posture: it is what makes chat's output auditable. A
plan has a deterministic `plan_id`, a cost, and a provenance list, so a user can
see what a request would do before it does it, and what it did afterwards.

---

## 2. Input handling

One input field accepts four kinds of text, distinguished by shape:

| Kind | Example | Route |
| --- | --- | --- |
| Canonical address | `model.layers[10].self_attention.query_projection.weight` | Direct resolution |
| Alias / selector | `Q[10]`, `K[10][0:256,0:256]`, `MLP.down[24][:]` | Alias resolution; candidates on ambiguity |
| WeightQL | `show tensor("Q[10]") @ transpose(tensor("K[10]"))` | Parse → plan |
| Natural language | `Show the L2 norm of every query projection` | Intent → WeightQL → plan |

Classification is by parse attempt, in that order, not by heuristics on the
string. If the WeightQL parser accepts it, it is WeightQL. Only text that no
parser accepts is treated as natural language.

### Required examples

All from the task specification §21, each with the WeightQL it must produce:

| Input | Produced WeightQL |
| --- | --- |
| `Show Q[10].` | `show tensor("Q[10]")` |
| `Show layer[10].attention.Q.` | resolve contextual → `show tensor("Q[10]")` |
| `Open model.layers[10].self_attn.q_proj.weight.` | `show tensor("model.layers[10]…")` |
| `Show Q[10][100, :].` | `SELECT slice FROM tensor("Q[10]") ROWS 100:101` |
| `Compare Q[10][100, :] with Q[20][100, :].` | `show compare(tensor("Q[10]")[100,:], tensor("Q[20]")[100,:]) by cosine_similarity` |
| `Visualize Q[10][0:128, :] @ K[10][:, 0:128].` | `show tensor("Q[10]")[0:128,:] @ tensor("K[10]")[:,0:128]` |
| `Show the L2 norm of every query projection.` | `SELECT layer_index, l2_norm(weight) FROM model(…).tensors WHERE role = "attention_query_projection" GROUP BY layer_index` — needs `WQL-007` (`QM-0072`) |

The last one is the useful test of the boundary: it is the only example that
needs a language feature that does not exist yet, and the honest response is to
say so with the requirement ID rather than to approximate it with a loop of
per-layer queries.

---

## 3. Candidate resolution

When an alias is ambiguous, the daemon returns `409` with candidates
(`API-007`). The UI must **present them, not choose**.

```text
"Att[10]" is ambiguous. 4 candidates:

  ▸ Q  model.layers[10].self_attention.query_projection.weight   [4096,4096] F32
  ▸ K  model.layers[10].self_attention.key_projection.weight     [1024,4096] F32
  ▸ V  model.layers[10].self_attention.value_projection.weight   [1024,4096] F32
  ▸ O  model.layers[10].self_attention.output_projection.weight  [4096,4096] F32

  Current selection is Q[10] — use it?   [use Q]  [choose…]
```

The current viewer selection may be *offered* as a default, visibly, with the
reason stated. It is never applied silently. `ARCHITECTURE.md` §15's worked
example does exactly this: *"Use current UI selection: Q projection"* is step 3 of
a plan the user can read, not an invisible inference.

---

## 4. Cost preview

Every plan shows its cost before execution.

```text
Plan  plan:b3:7a2f…
  reads      512 KiB   from 2 tensors
  host       768 KiB
  gpu        768 KiB
  backend    cpu-reference
  fidelity   ▣ exact
  result     [256, 256] F32

  [ Execute ]   [ Cancel ]   [ Copy WeightQL ]
```

| Cost | UI |
| --- | --- |
| Below `WARN_READ_BYTES` (64 MiB) | Execute is enabled; cost shown compactly |
| Above the warning | Execute requires a second confirm; the cost is emphasised |
| Above `MAX_READ_BYTES` (4 GiB) | Execute is **disabled**; the UI suggests a narrower slice |
| Whole-tensor read | Refused categorically, with the reason and a suggested slice |

Auto-execution is allowed only for metadata-tier plans — those that read nothing.
Everything else waits for a click.

---

## 5. KaTeX rendering

`katex-preview.ts` already renders the **parser's grouping**, not the source
order — the test is named `renders_the_grouping_the_parser_chose_not_the_source_order`.
That is the whole point of the preview: a user who typed something that parses
differently from how they read it should see the difference before executing.

```text
QK^\top                       C_{ij} = \sum_k A_{ik}B_{kj}                \lVert W \rVert_2
```

### Sanitization

`SEC-006`, task `QM-0075`. KaTeX renders user-influenced text, so the contract
must be explicit rather than assumed:

| Control | Setting |
| --- | --- |
| Source | **Never the user's raw string.** LaTeX is generated from the validated AST |
| `trust` | `false` — disables `\href`, `\url`, `\includegraphics` |
| `strict` | `"error"` — no silent fallbacks that could change meaning |
| `maxSize`, `maxExpand` | Bounded — blocks macro-expansion denial of service |
| `throwOnError` | `true`, caught and shown as a message |
| Output | Rendered into a container with no `innerHTML` assembly of user text |
| CSP | No inline event handlers; KaTeX's fonts served locally |

Because the LaTeX is generated from the AST, a string the parser rejected never
reaches KaTeX at all. That is a stronger property than escaping: the dangerous
input does not get as far as the renderer.

---

## 6. Progress and cancellation

Long operations need three things: visible progress, a working cancel, and a
truthful partial result.

| Aspect | Design |
| --- | --- |
| Transport | Server-Sent Events on `GET /v1/jobs/{jobId}/events`; polling fallback (`ADR-CANDIDATE-011`) |
| Progress fields | Phase, current tensor, blocks done/total, bytes read/written, elapsed, ETA where meaningful |
| Cancel | `POST /v1/query/{planId}/cancel` or `/v1/jobs/{jobId}/cancel`; acknowledged, not fire-and-forget |
| Latency | Bounded by one block — cancellation is checked between blocks |
| Partial results | Shown, **labelled partial**, with what was and was not covered |
| Cleanup | Temporary files removed; the completed-block manifest kept for resume |

A cancel that leaves the UI spinning is worse than no cancel, because the user
loses the ability to reason about what the system is doing. The acknowledgement
is part of the contract.

---

## 7. Result rendering

| Result | Rendering |
| --- | --- |
| Scalar | The value, its address, its byte offset, and its fidelity badge |
| Slice | A table for small slices; the matrix workspace for anything 2-D |
| Matrix | Opened in the workspace on the shared grid |
| Statistics | Table plus histogram, with `algorithm_version` and `approximate` shown |
| Comparison | Metric value, plus both operands' addresses |
| Plan only | The cost card of §4 |
| Error | The message, the caret, and the requirement ID when the gap is declared |

Every one carries a fidelity badge from the shared vocabulary
([`DATA_ARCHITECTURE.md`](DATA_ARCHITECTURE.md) §8). The task specification §21
requires the result UI to label exact, approximate, sampled, quantized, and
statistical-interpretation results distinctly; the badge vocabulary is that
labelling, shared with the viewer so the same word never means two things.

---

## 8. History and context

* **History** — every submitted query, its plan ID, its cost, and whether it
  executed. Re-runnable and copyable.
* **Suggested selectors** — derived from the current model's catalog: roles that
  exist, layer range, common aliases. Not a language model's guess; a catalog
  query.
* **Current selection context** — what is selected in the viewer or workspace,
  shown persistently, because it is what a contextual selector resolves against.
  A user must be able to see the context that will disambiguate their input
  *before* they type it.

---

## 9. Requirements

| ID | Requirement | State | Task |
| --- | --- | --- | --- |
| `CHAT-001` | Chat assistant: plan + cost before execution | Not started | `QM-0074` |
| `CHAT-002` | Client-side WeightQL parser matching Rust | ✓ Verified | verify only |
| `CHAT-003` | KaTeX preview of the parsed AST | ✓ Verified | verify only |
| `CHAT-004` | Error caret at the offending character | ✓ Verified | verify only |
| `CHAT-005` | Candidate resolution UI; never a silent pick | New | `QM-0075` |
| `SEC-006` | KaTeX sanitization contract | New | `QM-0075` |
| `API-011` | Query cancellation, acknowledged | New | `QM-0073` |
