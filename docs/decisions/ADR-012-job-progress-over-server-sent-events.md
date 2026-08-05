# ADR-012 — Job progress reaches the browser over Server-Sent Events

**Status:** Accepted
**Date:** 2026-08-04
**Implements:** `API-010`
**Promoted from:** `.plan/decisions/ADR-CANDIDATE-011-daemon-transport.md`

## Context

Request/response is settled and built: `crates/q-daemon` is axum over tokio,
with eight routes serving real data and five returning `501` carrying a
requirement ID. What is not settled is how **progress** reaches the browser
during a conversion that may run for hours.

* `Cargo.toml:59-61` — `axum = "0.7"`, `tower = "0.5"`, `tokio` with
  `rt-multi-thread`, `macros`, `net`, `io-util`, `signal`. **No WebSocket
  feature is enabled anywhere in the workspace.**
* `apps/web/model-viewer/src/tile-client.ts` — `fetch`-based, and treats a `501`
  as a declared gap rather than an error.
* `crates/q-catalog/src/job.rs` — the job state machine, its counters, and its
  persistence already exist and are verified (`JOB-001`, `JOB-003`). Nothing
  executes a job (`JOB-002`, stub, `501`), so nothing emits progress yet.
* `.plan/SECURITY_MODEL.md` §4 — T3, "a malicious page in another tab drives the
  local daemon", is the live threat. Its mitigations are a `127.0.0.1` bind, an
  explicit CORS allowlist that is **never `*`**, and request limits (`SEC-007`,
  `QM-0075`).

## Decision

**Progress is delivered over Server-Sent Events on plain HTTP.** Control actions
are ordinary `POST`s.

```text
GET  /v1/jobs/{jobId}/events     text/event-stream
POST /v1/jobs/{jobId}/cancel     acknowledged, not fire-and-forget
POST /v1/jobs/{jobId}/resume
GET  /v1/jobs/{jobId}            the authoritative job record
```

Four points the transport choice alone does not settle, decided here:

1. **The job record is the truth; the stream is a hint.** `GET /v1/jobs/{jobId}`
   returns the same state the stream reports, and a client may use it alone.
   This is what makes polling a genuine fallback rather than a slogan.
2. **The daemon buffers no events and implements no `Last-Event-ID` replay.**
   `EventSource` reconnects automatically, and a reconnecting client re-reads
   `GET /v1/jobs/{jobId}` for authoritative state before resuming the stream. A
   replay buffer would be a second, weaker copy of the job record, sized by
   guesswork and wrong after a daemon restart. `QM-0033`'s error handling
   already says a client disconnect ends only the stream, never the job.
3. **Cancellation is a `POST`, and it is acknowledged.** SSE is one-directional
   and this is the right shape: the client asks, the executor stops at a block
   boundary, and the state change is observable in the job record whether or not
   any stream is open. A cancellation that travelled down the same socket as
   progress would be unobservable to a client that had reconnected.
4. **One executor means one stream.** Additional job requests are queued, not
   spawned (`QM-0033`), so the browser holds at most one event stream per
   daemon.

## Alternatives considered

**WebSocket.** Bidirectional, with lower per-message overhead. Rejected on three
counts, of which the third decides it: nothing in the traffic shape is
bidirectional — progress flows one way and control actions are discrete; it
requires a new tokio/axum feature plus framing, heartbeats, and reconnection
logic written by hand; and it is **a second authentication and origin story**
opened exactly where `SECURITY_MODEL.md` T3 is the live risk. A WebSocket
upgrade does not inherit the CORS allowlist, and the browser sends no `Origin`
enforcement the server can lean on — every mitigation `QM-0075` builds for HTTP
would need building again, differently, for one socket.

**Polling `GET /v1/jobs/{jobId}`.** The simplest possible answer, and no new
route type. Rejected as the *primary* mechanism because it has no good latency
setting: a 1-second poll across a four-hour job is 14 400 requests for a handful
of state changes, and a 30-second poll makes the UI feel broken. It is kept as
the fallback, where its weaknesses do not matter.

**gRPC-web.** Typed contracts and real streaming. Rejected: it needs a proxy, a
codegen step, and a large dependency tree, for a service that runs on
`127.0.0.1` and talks to one browser tab. The typing it buys is already bought
by the JSON Schemas in `schemas/`.

## Why the security argument is the deciding one

The other arguments — familiarity, dependency weight, traffic shape — are
preferences. This one is a property.

SSE is plain HTTP. The event stream is a `GET` that inherits, without any
additional code, the CORS allowlist, the `127.0.0.1` bind restriction, and the
request limits that `QM-0075` implements once for every other route. There is no
second path into the daemon to secure, audit, or forget to secure.

That property survives contact with the rest of the system: a stream is subject
to the same origin policy as `GET /v1/models`, and a future reviewer checking
"what can a hostile page reach" reads one allowlist rather than two mechanisms.

## Consequences

* `.plan/API_CONTRACTS.md` §1's four new job routes and its §3 event shapes
  (`event: progress`, `event: complete`) are now backed by a decision rather
  than by a recommendation. They need no change.
* **No manifest change is required.** Verified on this machine:
  `cargo tree -p q-daemon -e features -i axum` reports `axum v0.7.9` with the
  `tokio` feature already enabled through `axum`'s defaults, which is the
  feature `axum::response::sse` is gated behind. SSE costs zero new
  dependencies; a WebSocket would have cost at least one new feature and a
  framing implementation.
* Progress events are throttled to ≤ 10/s (`QM-0033`), and the stream carries
  periodic keep-alives so an idle phase is not mistaken for a dead connection.
* **The HTTP/1.1 six-connections-per-origin limit is a documented constraint,
  not a bug to discover later.** One executor means one stream today. A future
  multi-job UI opening one stream per job would exhaust the budget at six over
  HTTP/1.1; the answers are HTTP/2, or one multiplexed stream carrying a
  `job_id` per event. Recorded now so that the choice is made deliberately.
* Graceful shutdown closes streams rather than dropping them: `tokio`'s `signal`
  feature is already enabled, and `QM-0033` pauses an in-flight job on `SIGINT`
  rather than losing it.
* `QM-0073` (query cancellation) and `QM-0075` (origin policy) inherit this
  decision. `QM-0073`'s `POST /v1/query/{planId}/cancel` follows the same
  acknowledged-`POST` shape as job cancellation, and `499` already has a place
  in the status-code contract.

### What this does not unblock

`QM-0033`'s `## Dependencies` section names `QM-0032` and `QM-0022`; it cites
`ADR-CANDIDATE-011` under `Repository Evidence`, not as a dependency. This ADR
removes the decision risk from the transport its `Scope` assumes; `QM-0033`
remains gated on `QM-0032` reaching `Complete`.

## Research

* **MDN — Using server-sent events** —
  https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events,
  retrieved 2026-08-04. Confirms that `EventSource` reconnects automatically
  unless `close()` is called, that the `retry:` field sets the reconnection
  interval, that an `id:` field sets the last-event-ID the browser replays on
  reconnect, and that browsers cap SSE connections at **six per origin over
  HTTP/1.1** (negotiated, default 100 streams, over HTTP/2), an issue marked
  "won't fix" in both Chrome and Firefox. *Credibility: MDN is the reference
  documentation for browser web APIs.*

  This **changed the decision's detail, not its direction.** Automatic
  reconnection is the reason point 2 above exists at all: because the browser
  will reconnect on its own, the server has to state whether it replays. It
  does not — the job record does. Without this, an implementer could reasonably
  have built an event buffer nobody asked for.

* **axum 0.7 `response::sse` module documentation** —
  https://docs.rs/axum/0.7/axum/response/sse/index.html, retrieved 2026-08-04.
  Confirms `Sse`, `Event`, and `KeepAlive`, and that the module is gated behind
  axum's `tokio` cargo feature. *Credibility: the crate's own generated API
  documentation.* The feature claim was then **verified locally** with
  `cargo tree` rather than trusted — see Consequences.

Neither source overrides anything in `ARCHITECTURE.md` or the `.plan` corpus,
both of which already name SSE. They supply behavioural detail the repository
does not contain.
