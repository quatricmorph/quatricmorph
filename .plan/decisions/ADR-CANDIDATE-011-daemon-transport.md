# ADR-CANDIDATE-011 — Local daemon transport

## Status

`Open`.

## Context

Request/response is settled — HTTP, built and tested. What is not settled is how
**progress** reaches the browser during a conversion that may run for hours.

## Repository evidence

* `crates/q-daemon/` — axum 0.7 over tokio, 941 lines, 8 tests. 8 routes live,
  5 × 501.
* `Cargo.toml` — `axum`, `tower`, `tokio` with `rt-multi-thread`, `macros`,
  `net`, `io-util`, `signal`. **No WebSocket feature enabled.**
* `apps/web/model-viewer/src/tile-client.ts` — `fetch`-based, treats 501 as a
  declared gap.
* `STATUS.md` `JOB-002` — job runner is a stub; nothing emits progress yet.
* `q_catalog::job` — the state machine and its counters already exist.

## Decision required

How does job progress reach the browser?

## Options

| Option | |
| --- | --- |
| **A** | HTTP + Server-Sent Events on `GET /v1/jobs/{jobId}/events` |
| **B** | HTTP + WebSocket |
| **C** | HTTP + polling `GET /v1/jobs/{jobId}` |
| **D** | gRPC-web |

## Advantages

* **A** — one-directional, which is exactly the traffic shape; plain HTTP, so it
  inherits the CORS policy and the origin checks; auto-reconnect is built into
  `EventSource`; axum supports it natively with no new dependency.
* **B** — bidirectional; lower per-message overhead.
* **C** — simplest possible; no new route type.
* **D** — typed contracts, streaming.

## Disadvantages

* **A** — one-directional. Cancellation needs a separate `POST`, which is fine
  and arguably clearer.
* **B** — a new tokio feature, framing, heartbeats, and reconnection logic, for
  bidirectionality nothing needs; and **a second authentication and origin story**
  right where [`SECURITY_MODEL.md`](../SECURITY_MODEL.md) T3 is the live risk.
* **C** — latency versus request volume trade-off with no good answer; a 1 s poll
  over a 4-hour job is 14 400 requests.
* **D** — a proxy, a codegen step, and a large dependency for a local service.

## Risks

* **A** — SSE over HTTP/1.1 is subject to the 6-connections-per-origin limit.
  Mitigated: the MVP has **one** job executor, so one stream. Documented so that
  a future multi-job UI does not discover it by surprise.
* Long-lived connections and daemon shutdown. Mitigated: `tokio` already has the
  `signal` feature; graceful shutdown closes streams.

## Recommended default

**A.** HTTP + Server-Sent Events.

```text
GET  /v1/jobs/{jobId}/events     text/event-stream
POST /v1/jobs/{jobId}/cancel     acknowledged, not fire-and-forget
POST /v1/jobs/{jobId}/resume
```

Polling stays available as a fallback for any client that cannot use
`EventSource`, since `GET /v1/jobs/{jobId}` returns the same record.

The deciding argument is the security one: SSE is plain HTTP, so it inherits the
CORS allowlist, the bind-address restriction, and the request limits without a
second implementation of any of them. A WebSocket would need all three again.

## Tasks affected

`QM-0033` (implements), `QM-0073` (query cancellation), `QM-0075` (origin
policy).

## Decision deadline

Before `QM-0033`.
