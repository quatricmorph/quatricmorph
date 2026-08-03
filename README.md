# Quatricmorph

Local-first platform for inspecting, querying, morphing, and verifying open-weight models.

## Start here

- Agent guide: [AGENTS.md](AGENTS.md)
- Docs index: [docs/README.md](docs/README.md)
- Prerequisites gate: [docs/requirements/PREREQUISITES.md](docs/requirements/PREREQUISITES.md)
- Active viz MVP: [docs/requirements/VIZ_MVP.md](docs/requirements/VIZ_MVP.md)
- Full architecture: [docs/PRODUCT_ARCHITECTURE_v1.md](docs/PRODUCT_ARCHITECTURE_v1.md)

## App (`quatricmorph/`)

```bash
cd quatricmorph
npm install
npm run dev      # http://127.0.0.1:5173
npm test
npm run build
```

## Layout

```text
quatricmorph/   Vite + TypeScript Three.js visualizer
mm/             Original mm reference (do not delete)
docs/           Product, requirements, agent charter
.cursor/rules/  Cursor agent rules
prompts.md      Visualization MVP engineering brief
```
