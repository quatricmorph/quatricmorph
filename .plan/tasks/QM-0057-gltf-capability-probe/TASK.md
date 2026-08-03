# QM-0057 — glTF extension capability probe and fallback

## Status

Blocked

Unblocks when `QM-0051` reaches `Complete`.

## Phase

Phase 05 — Cesium model viewer

## Objective

Determine at runtime which glTF extensions actually work, and select the emission
profile accordingly — rather than assuming support.

## Repository Evidence

* `ARCHITECTURE.md` §10.2: *"Quatricmorph must check the renderer's actual
  support level and have its own fallback."*
* §11.3: Cesium's `CustomShader` *"is currently marked experimental … should not
  become a long-term core dependency."*
* `QM-0042` emits profiles A, B, and C; `QM-0043` adds the extensions used by A.
* `ADR-CANDIDATE-017` — the three-profile ladder, floor = core glTF 2.0.

## Requirements Covered

`CESIUM-010`.

## Dependencies

`QM-0051`, `QM-0043`.

## Blocks

`QM-0080`.

## Parallelization

Lane B, parallel with `QM-0054`…`QM-0056`.

## Program Boundary

`apps/web/model-viewer`; the daemon's profile selection.

## Scope

* Probe GLBs — 3 instances each — one per extension, checked in.
* At viewer start, load each and record: **loaded**, **rendered**, and
  **silently produced nothing**.
* Select the highest working profile; request tiles in that profile.
* Show the active profile in the dev panel.
* Cache the probe result for the session.

## Out of Scope

Implementing the profiles (`QM-0042`, `QM-0043`) · `CustomShader` beyond a flag ·
probing non-glTF capabilities.

## Files Expected to Change

* `apps/web/model-viewer/src/cesium/tileset.ts`
* `crates/q-daemon/src/lib.rs` — accept a profile parameter on the tile route

## Files Expected to Add

* `apps/web/model-viewer/src/cesium/capability-probe.ts`
* `apps/web/model-viewer/fixtures/probe/{instancing,features,metadata}.glb`
* `apps/web/model-viewer/src/__tests__/capability-probe.test.ts`

## Files Expected to Remove or Deprecate

None.

## Data Contracts

```jsonc
{ "ext_mesh_gpu_instancing":   { "loaded": true, "rendered": true },
  "ext_mesh_features":         { "loaded": true, "rendered": true },
  "ext_structural_metadata":   { "loaded": true, "rendered": false },
  "selected_profile": "B",
  "reason": "EXT_structural_metadata loaded but produced no readable properties" }
```

**"Loaded" and "rendered" are separate checks** because the dangerous failure is
the silent one: a loader that accepts the file and draws nothing. A boolean
"supported" would miss it.

## Memory and Performance Constraints

Probe GLBs are 3 instances each, a few kilobytes. The probe adds < 200 ms to
startup and runs once per session.

## Implementation Plan

1. Author three minimal probe GLBs, one per extension, each with 3 instances at
   known positions.
2. Load each; assert primitive count, instance count, and — for metadata — that
   a known property reads back with its expected value.
3. Select the highest fully working profile.
4. Pass `?profile=A|B|C` on tile requests; the daemon serves that profile.
5. Show the result and the reason in the dev panel.
6. Test with each extension force-disabled.

## Error Handling

* A probe failing to load → that extension is unsupported; continue.
* A probe loading but rendering nothing → **unsupported**; this is the case the
  probe exists for.
* All probes failing → profile C, which uses no extension and therefore cannot
  fail for extension reasons.
* The daemon lacking a requested profile → 404 naming the profile; the viewer
  falls back one level.

## Acceptance Criteria

1. The probe runs at startup and completes in < 200 ms.
2. Each extension is separately reported as loaded and rendered.
3. The highest fully working profile is selected.
4. Force-disabling `EXT_structural_metadata` selects profile B.
5. Force-disabling instancing selects profile C.
6. Profile C renders correctly with no extension.
7. The active profile and reason appear in the dev panel.
8. Tile requests carry the selected profile.
9. Picking works in **all three** profiles.

## Verification Plan

**Automated** — vitest for selection logic; Playwright loading each probe and
asserting render results, plus a picking test per profile.
**Manual** — inspect the dev panel; force-disable extensions and observe the
fallback.

## Suggested Commands

```bash
npx playwright test apps/web/model-viewer/e2e/capability-probe.spec.ts   # new
curl -s "localhost:PORT/v1/visualizations/<m>/tiles/<t>.glb?profile=C" -o /tmp/c.glb
```

## Test Cases

| Input | Expected |
| --- | --- |
| All extensions working | Profile A |
| Metadata renders nothing | Profile B; reason recorded |
| Instancing unsupported | Profile C |
| All probes fail | Profile C; still renders |
| Profile C tile | Renders; picking resolves via the daemon |
| Probe timing | < 200 ms |
| Daemon lacks profile A | 404 naming it; viewer falls back |
| Picking in each profile | Correct address in all three |

## Risks

| Risk | Mitigation |
| --- | --- |
| A probe passes but real tiles fail at scale | Probes mirror real tile structure; `QM-0080` exercises real tiles |
| Silent extension failure | Loaded and rendered are checked separately — the whole point |
| Profile C is never exercised | It is an explicit test case, not only a fallback |

## Completion Evidence

* Probe report for the development browser.
* Screenshots rendering under each of the three profiles.
* Picking results per profile.
* Probe timing.
