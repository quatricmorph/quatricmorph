# query-interface — the WeightQL input surface

A text input, a live parse, and a KaTeX preview of the expression the parser
actually understood. Nothing else.

## What this is not

It is **not a chat interface** (`CHAT-001`). ARCHITECTURE.md §15 is specific
about what a chat assistant must do — build a plan, show the estimated I/O, and
never touch weight bytes directly — and none of that exists yet. Shipping a
text box that *looked* conversational while doing none of it would be the exact
failure mode §20 forbids.

## What it does do

* Tokenizes and parses WeightQL client-side, mirroring `crates/q-weightql`.
  A syntax error is reported at the character, before anything is sent.
* Renders the parsed AST as KaTeX so the user can see whether
  `Q[10] @ transpose(K[10])` was read as they meant it.
* Refuses the same constructs the Rust parser refuses — `eval`, user-defined
  functions, shell interpolation, raw SQL — so a rejection is consistent
  whether it happens in the browser or the daemon.

Sending the query to `POST /v1/query` is the daemon's job; this app formats and
validates, and the daemon is the authority.

## Why the grammar exists twice

Client-side parsing gives immediate feedback without a round trip, and the
KaTeX preview needs an AST. `src/__tests__/parity.test.ts` pins the two
implementations to the same accept/reject decisions on a shared corpus, so a
divergence fails a test rather than surfacing as a confusing UI.
