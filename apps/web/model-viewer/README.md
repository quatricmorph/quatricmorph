# model-viewer — Visualization Plane

**App shell only.** There is no CesiumJS dependency, no tileset, and no
rendering in this pass (`CESIUM-001`).

## Why it exists as a shell

ARCHITECTURE.md §12.1 assigns tile traversal, camera, LOD, picking, and
selection to a CesiumJS prototype. None of that can be built before there is a
`tileset.json` to traverse, and there is not — `q-tileset` returns
`NotImplemented`, and `GET /v1/visualizations/{id}/tileset.json` returns 501.

What this directory *does* contain is the client-side contract the viewer will
hold to, written as real, tested code:

* `src/lod-policy.ts` — the §9.3 loading rules as a pure decision function:
  which LOD a camera distance implies, and — critically — that zooming out must
  never request exact values.
* `src/tile-client.ts` — the daemon endpoints the viewer will call, with the
  501 responses handled as first-class outcomes rather than errors to retry.

Both are unit-tested. Neither renders anything.

## What must be true before this becomes a viewer

1. `q-tiles` builds a `.qtile` pyramid for a model (`TILE-004`).
2. `q-tileset` emits a real `tileset.json` (`CESIUM-001`).
3. `q-gltf` emits GLB tile content (`GLB-001`).
4. Only then: add CesiumJS, load the tileset, and wire picking to
   `GET /v1/tensors/{id}/value`.

Until then this app deliberately shows nothing rather than a plausible scene
built from invented data.
