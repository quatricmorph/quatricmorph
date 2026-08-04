# QM-0075 — Candidate UI, KaTeX sanitization, and origin policy

## Status

Deferred

Not in v1 — post-v1 **platform release**. See [`STRATEGY_ALIGNMENT.md`](../../STRATEGY_ALIGNMENT.md) and [`PRODUCT_SCOPE.md`](../../PRODUCT_SCOPE.md) §4. The specification below remains correct; only its release has moved.

## Phase

Phase 07 — WeightQL and chat

## Objective

Render candidates without choosing, sanitize KaTeX properly, and close the
daemon's origin boundary.

## Repository Evidence

* `apps/web/query-interface/src/katex-preview.ts` — `CHAT-003` Verified. **No
  stated sanitization contract.**
* `an_ambiguous_alias_is_a_409_carrying_its_candidates` (`API-007` Verified) —
  the data exists; nothing renders it.
* `QM-0023` fixed the candidate payload shape.
* `crates/q-daemon` — binds a socket; **no CORS decision recorded**.
* `docs/CURRENT_ARCHITECTURE.md` §5 — `mm` reached `eval` from a URL parameter;
  that is why `q-expression` is a closed enum.
* `SECURITY_MODEL.md` T3 — any page the user visits can reach `127.0.0.1`.

## Requirements Covered

`CHAT-005`, `SEC-006`, `SEC-007`, `MVP-34`, `MVP-37`.

## Dependencies

`QM-0073`, `QM-0023`.

## Blocks

`QM-0085`, `QM-0094`.

## Parallelization

Parallel with `QM-0074`. Touches the query interface and the daemon.

## Program Boundary

`apps/web/query-interface`, `crates/q-daemon`.

## Scope

* Candidate list UI on 409: all candidates, ordered, **never pre-selected**.
* The current selection may be *offered* as a default, visibly, with a stated
  reason.
* The KaTeX sanitization contract, asserted by tests.
* Daemon: bind to `127.0.0.1`, a CORS allowlist, request size limits.
* Cost preview card with the execute/cancel gate.

## Out of Scope

Chat intent mapping (`QM-0074`) · new query capability · authentication, which is
deliberately absent for a local-first tool.

## Files Expected to Change

* `apps/web/query-interface/src/katex-preview.ts`
* `apps/web/query-interface/src/app.ts`
* `crates/q-daemon/src/lib.rs`
* `crates/q-daemon/src/main.rs`

## Files Expected to Add

* `apps/web/query-interface/src/candidates.ts`
* `apps/web/query-interface/src/cost-card.ts`
* `apps/web/query-interface/src/__tests__/katex-security.test.ts`
* `crates/q-daemon/tests/origin_policy.rs`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

**KaTeX configuration, asserted by test:**

```ts
{ trust: false,        // no \href, \url, \includegraphics
  strict: 'error',     // no silent fallbacks
  maxSize: 50, maxExpand: 100,
  throwOnError: true }
```

**The LaTeX is generated from the validated AST, never from the user's raw
string.** A string the parser rejected never reaches KaTeX at all — stronger than
escaping, because the dangerous input does not get as far as the renderer.

**CORS:** an explicit allowlist of local dev origins. **Never `*`.** Without an
origin policy, any page the user visits could enumerate their models and read
their weights.

## Memory and Performance Constraints

* KaTeX `maxExpand: 100` bounds macro-expansion denial of service.
* Request body and query-string limits on the daemon.
* Candidate lists are **never truncated**; the list scrolls.

## Implementation Plan

1. `candidates.ts`: render the 409 list with all nine fields per candidate; a
   "use current selection" button that states the reason.
2. `cost-card.ts`: cost, tier, fidelity, execute/cancel, second confirm above the
   warning threshold, disabled above the refusal threshold.
3. Apply the KaTeX configuration; generate LaTeX from the AST only.
4. Daemon: `127.0.0.1` default, `--bind` requiring an explicit flag plus a
   warning, CORS allowlist, body limits.
5. Tests for each control.

## Error Handling

* Zero candidates → 404 path, not an empty list.
* One candidate → resolved, no list shown.
* KaTeX throwing → the message is displayed; **no partial render**, which could
  show a formula different from the one that would execute.
* A disallowed origin → 403 with no detail beyond the refusal.
* A body over the limit → 413.

## Acceptance Criteria

1. A 409 renders every candidate with all nine fields, ordered deterministically.
2. **No candidate is pre-selected.**
3. "Use current selection" states its reason before applying.
4. KaTeX runs with `trust: false`, `strict: 'error'`, bounded `maxExpand` —
   asserted.
5. LaTeX is generated from the AST; a raw string never reaches KaTeX — asserted.
6. `\href{javascript:...}{x}` does not produce a link.
7. A macro-expansion bomb is bounded, not hung.
8. The daemon binds `127.0.0.1` by default; `0.0.0.0` requires a flag **and**
   warns.
9. A request from a disallowed origin gets 403; CORS is never `*`.
10. The cost card gates execution at both thresholds.

## Verification Plan

**Automated** — `katex-security.test.ts` for every configuration and attack
string; `origin_policy.rs` for bind address, CORS, and limits; vitest for the
candidate UI.
**Manual** — a cross-origin request from a scratch page must be refused.

## Suggested Commands

```bash
cd apps/web && npx vitest run katex-security                       # introduced here
cargo test -p q-daemon --test origin_policy                         # introduced here
curl -H "Origin: http://evil.example" localhost:PORT/v1/models -v
```

## Test Cases

| Input | Expected |
| --- | --- |
| 409 with 4 candidates | All 4 shown; none selected |
| Candidate order | Deterministic across runs |
| "Use current selection" | Reason shown before applying |
| `\href{javascript:alert(1)}{x}` | No link produced |
| A deeply nested macro | Bounded by `maxExpand`; no hang |
| A parser-rejected string | Never reaches KaTeX |
| Daemon default bind | `127.0.0.1` |
| `--bind 0.0.0.0` | Works **and warns** |
| `Origin: http://evil.example` | 403 |
| CORS header | Never `*` |
| 100 MiB request body | 413 |
| Cost 100 MiB | Second confirm required |
| Cost 8 GiB | Execute disabled |

## Risks

| Risk | Mitigation |
| --- | --- |
| CORS relaxed to `*` for local development convenience | An explicit test asserts it is never `*` |
| KaTeX configuration drifts | Configuration asserted by test, not by convention |
| A candidate is auto-selected to reduce clicks | "None pre-selected" is an acceptance criterion |

## Completion Evidence

* Candidate list screenshot with nothing pre-selected.
* KaTeX security test output including each attack string.
* Origin-policy test output.
* The cross-origin `curl` transcript.
* Cost-card screenshots at both thresholds.
