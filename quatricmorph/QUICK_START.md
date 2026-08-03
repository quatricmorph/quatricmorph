# Quatricmorph Quick Start Guide

## Prerequisites

- Rust 1.70+
- Node.js 18+
- Python 3.9+ (for Python bindings)

## Building the Workspace

### 1. Rust Crates

```bash
cd quatricmorph

# Build all crates
cargo build

# Build specific crate
cargo build -p q-cli

# Build with optimizations
cargo build --release

# Run tests
cargo test
```

### 2. Web Application

```bash
cd quatricmorph/apps/web

# Install dependencies
npm install

# Development server (http://localhost:3000)
npm run dev

# Production build
npm run build

# Preview build
npm run preview
```

### 3. Python Bindings

```bash
cd quatricmorph/python

# Development install
pip install -e .

# Install from source
pip install .
```

## Running Applications

### CLI Tool

```bash
cd quatricmorph
cargo run -p q-cli -- --help
cargo run -p q-cli -- models
cargo run -p q-cli -- run --model llama
```

### Daemon

```bash
cd quatricmorph
cargo run -p q-daemon
```

### Web Viewer

```bash
cd quatricmorph/apps/web
npm run dev
# Open http://localhost:3000
```

## Project Structure Quick Reference

| Directory | Purpose |
|-----------|---------|
| `crates/` | Rust library and binary crates |
| `apps/web/` | React/TypeScript web application |
| `apps/desktop/` | Desktop application (placeholder) |
| `gpu/` | GPU shader implementations (WGPU, CUDA, Metal) |
| `architectures/` | Model architecture definitions |
| `schemas/` | JSON schema definitions |
| `python/` | Python package and bindings |
| `fixtures/` | Test data and examples |

## Crate Dependencies

```
q-cli          → q-daemon, q-architecture, q-catalog
q-daemon       → q-tensor-runtime, q-gpu, q-cache
q-tensor-runtime → q-tiles, q-statistics, q-safetensors
q-gpu          → q-tiles, q-cache (with wgpu/cuda/metal features)
q-catalog      → q-architecture, q-safetensors
```

## Key Files

- `Cargo.toml` - Workspace configuration
- `quatricmorph/apps/web/package.json` - Web app dependencies
- `quatricmorph/architectures/*/config.toml` - Model configs
- `quatricmorph/schemas/*/schema.json` - Data format schemas

## Troubleshooting

### Cargo build fails
- Ensure Rust is up to date: `rustup update`
- Clean build: `cargo clean && cargo build`

### Web app won't start
- Clear node_modules: `rm -rf node_modules && npm install`
- Check Node version: `node --version` (should be 18+)

### Python import errors
- Install in development mode: `pip install -e .`
- Ensure Python version: `python --version` (should be 3.9+)

## Development Workflow

1. Make changes to crates
2. Run `cargo check` for quick validation
3. Run `cargo test` for unit tests
4. Build with `cargo build --release`
5. Update web app in `apps/web/`
6. Test integration between components

## Additional Resources

- [README.md](./README.md) - Project overview
- [GENERATED_STRUCTURE.md](./GENERATED_STRUCTURE.md) - Detailed structure
- [../docs/](../docs/) - Architecture and design documents
