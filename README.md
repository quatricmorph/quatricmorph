# Quatricmorph

Local-first platform for inspecting, querying, morphing, and verifying open-weight models.

**Active MVP (Track A):** browser visualization of a single multiplication `A @ B = C` in a shared 3D margin grid (`quatricmorph/`).

## Start here

- Agent guide: [AGENTS.md](AGENTS.md)
- Docs index: [docs/README.md](docs/README.md)
- Visualization architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Prerequisites gate: [docs/requirements/PREREQUISITES.md](docs/requirements/PREREQUISITES.md)
- Active viz MVP: [docs/requirements/VIZ_MVP.md](docs/requirements/VIZ_MVP.md)
- Full product architecture: [docs/PRODUCT_ARCHITECTURE_v1.md](docs/PRODUCT_ARCHITECTURE_v1.md)

## Visualization MVP (`quatricmorph/`)

Spatial Matrix Multiplication — enter compatible matrices, see planes A (I×K), B (K×J), C (I×J) on a shared 3D grid, orbit, hover values, select a result path, animate the output-cell dot product, and share a URL.

### Preview / develop / build

```bash
cd quatricmorph
npm install
npm run dev      # http://127.0.0.1:5173
npm test
npm run build
npm run preview
```

### Input format

Paste A/B values as comma- or space-separated rows (newlines or `;`):

```text
1, 2, 3
4, 5, 6
```

Presets: Random, Identity, Sequential, Zeros, Ones, Small Example (default `[[1,2,3],[4,5,6]] @ [[7,8],[9,10],[11,12]] → [[58,64],[139,154]]`).

B rows sync to A columns by default (optional unlock).

### Operations

| Control | Action |
| --- | --- |
| Play / Pause / Step / Reset Calculation | Output-cell dot-product animation |
| Reset View / Fit View / Camera preset | Orbit camera |
| Copy Share Link | URL with A/B values, display, camera |
| Click C cell | Highlight A row / B column / C cell |

### Share links

State is encoded in the query string (JSON or compressed flatten). Invalid URLs fall back to defaults with a validation message. Transient animation timers and Three.js objects are not serialized.

### Scope / limitations

- One expression: `A @ B = C` (matrix/vector/scalar shapes).
- No attention/QKV/LoRA, nested trees, broadcasting, batching, PyTorch import, or backends in the MVP UI.
- Interactive target ~32×32; point-sprite markers; research `mm` examples remain under `examples/` but are not the product surface.

### Attribution & licenses

Visualization core adapted from [mm](https://github.com/bhosmer/mm) (Meta Platforms, MIT — see `mm/LICENSE`). Retain that notice for derived Three.js visualization code. Quatricmorph product docs and new modules in this repository follow the project’s licensing.

## Layout

```text
quatricmorph/   Vite + TypeScript Three.js visualizer (MVP)
mm/             Original mm reference (read-only — do not delete)
docs/           Product, requirements, agent charter, architecture
.cursor/rules/  Cursor agent rules
prompts.md      Visualization MVP engineering brief
```
