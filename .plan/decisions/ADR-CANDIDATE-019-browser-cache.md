# ADR-CANDIDATE-019 — Browser caching strategy

## Status

`Open`.

## Context

`ARCHITECTURE.md` §13.1 defines five cache levels; L3 is *"Browser — Cache Storage
/ IndexedDB"*. The viewer will fetch hundreds of GLB and `.qtile` files.
Re-fetching them on every navigation would make the viewer feel broken over a
local socket that is otherwise fast.

## Repository evidence

* `crates/q-cache/src/lib.rs:349` — `L3BrowserCache`, and :376 `L4RemoteCache`;
  both refuse. `l3_and_l4_refuse_rather_than_missing_silently`.
* `STATUS.md` `CACHE-006` — **Stub**.
* `CACHE-001`…`CACHE-004` **Verified** for L1 and L2, including reopen survival.
* `CACHE-008` — *"The cache works; nothing calls it yet."*
* `apps/web/model-viewer/src/tile-client.ts` — plain `fetch`, no caching layer.
* `q_cache::CacheKey` — the §13.2 key, **excluding** the palette, because colour
  is applied in the shader.

## Decision required

How does the browser cache tiles for the MVP?

## Options

| Option | |
| --- | --- |
| **A** | HTTP caching — `ETag` / `Cache-Control` from the daemon; the browser's own cache does the work |
| **B** | Cache Storage API, managed by a service worker |
| **C** | IndexedDB, managed by the application |
| **D** | In-memory only, per session |

## Advantages

* **A** — **zero client code.** Tile URIs are content-addressed by `TileId`, so
  they are immutable and `Cache-Control: immutable` is honestly true. Cesium's own
  request scheduler benefits without knowing anything about it.
* **B** — offline capability; explicit eviction control; survives reload.
* **C** — large quota; structured queries over cached entries.
* **D** — trivial.

## Disadvantages

* **A** — eviction is the browser's decision, not ours; no offline story.
* **B** — a **service worker** brings a registration lifecycle, an update story,
  and a class of stale-content bug that is notoriously hard to debug — for a
  local-first app whose server is on the same machine.
* **C** — hand-written eviction, quota handling, and serialization; IndexedDB's
  API is awkward for blobs.
* **D** — every navigation re-fetches.

## Risks

* **A** — a browser evicting aggressively causes re-fetches. Over localhost that
  costs milliseconds, not a user-visible stall.
* **B** — a stale service worker serving old tiles after a regeneration is
  exactly the failure that makes users distrust a viewer, and `TileId` stability
  means the URL would not change to signal it.

## Recommended default

**A** for the MVP; **B** remains the L3 extension point.

```http
Cache-Control: public, max-age=31536000, immutable
ETag: "<tile_id>"
```

The `immutable` claim is **true by construction**: `TileId` is content-derived and
extent- and LOD-sensitive (`TILE-003`), so a changed tile is a different URL. That
is what makes the simplest option also the correct one here — it is not a
shortcut, it is a property of the ID scheme.

`.qtile` responses get the same treatment. `tileset.json` gets
`Cache-Control: no-cache` with an `ETag`, since a regeneration changes it in place.

`L3BrowserCache` keeps refusing, and `l3_and_l4_refuse_rather_than_missing_silently`
keeps passing — the extension point stays honest about not existing.

## Tasks affected

`QM-0044` (serves tileset with correct headers), `QM-0051` (client relies on HTTP
caching), `QM-0032` (server-side L1/L2 wiring).

## Decision deadline

Before **`QM-0032`**, the earliest task in `Tasks affected`.

Corrected from `QM-0051`, which named the task that *consumes* HTTP caching in
the browser rather than the tasks that first *commit* to it. `QM-0032` (Wave 2)
wires the server-side cache; `QM-0041` (Wave 3) commits the content-addressed
output layout `<out>/<model_id>/tiles/<tile_id>.qtile` that makes the `immutable`
claim true; `QM-0044` (Wave 3) emits the `Cache-Control` and `ETag` headers.
All three precede `QM-0051` (Wave 4). See `README.md` §"How a deadline is
derived".
