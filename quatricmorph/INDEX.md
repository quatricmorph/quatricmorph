# Quatricmorph Project Index

## 📖 Documentation

### Getting Started
- **[SETUP_COMPLETE.md](./SETUP_COMPLETE.md)** - Complete setup verification and summary
- **[QUICK_START.md](./QUICK_START.md)** - Build and run instructions (5 minutes to first run)
- **[README.md](./README.md)** - Project overview and structure

### Development
- **[DEVELOPMENT.md](./DEVELOPMENT.md)** - Detailed development guide
- **[GENERATED_STRUCTURE.md](./GENERATED_STRUCTURE.md)** - Full structure documentation

## 🏗️ Project Structure

### Rust Backend (`crates/`)
All 16 crates compile successfully with `cargo check --all`

#### Core Crates
- `q-source` - Source code parsing and management
- `q-architecture` - Architecture definitions
- `q-catalog` - Model registry and discovery
- `q-tensor-runtime` - Core tensor computation engine

#### Data & Format Crates
- `q-safetensors` - Model weight format support
- `q-nsir` - Neural Source Intermediate Representation
- `q-tiles` - Tile-based data structures
- `q-weightql` - Weight query language

#### Computation & Analysis
- `q-statistics` - Statistical analysis tools
- `q-expression` - Expression parsing and evaluation
- `q-gltf` - 3D model support (glTF format)
- `q-tileset` - Tileset management and organization
- `q-cache` - Caching layer for performance

#### GPU & System Crates
- `q-gpu` - GPU acceleration (WGPU, CUDA, Metal)
- `q-daemon` - Background service (runnable: `cargo run -p q-daemon`)
- `q-cli` - Command-line interface (runnable: `cargo run -p q-cli`)

### Web Frontend (`apps/web/`)
- React 18.2 + TypeScript + Vite
- Development server ready: `npm run dev`
- Production build ready: `npm run build`
- Three.js integration for 3D visualization

### GPU Implementations (`gpu/`)
- `wgsl/` - WebGPU shaders for browser compatibility
- `cuda/` - CUDA kernels for NVIDIA GPUs
- `metal/` - Metal shaders for macOS

### Model Architectures (`architectures/`)
Configuration templates for:
- `llama/` - Llama architecture
- `qwen/` - Qwen architecture
- `kimi/` - Kimi architecture
- `deepseek/` - DeepSeek architecture
- `generic/` - Custom/generic architectures

### Data Schemas (`schemas/`)
JSON Schema definitions for:
- `nsir/` - Neural Source Intermediate Representation
- `qtile/` - Quantized Tile format
- `weightql/` - Weight Query Language
- `visualization/` - Visualization data format

### Python Bindings (`python/`)
- `quatricmorph/` - Main Python package
- `setup.py` - Installation configuration
- `pyproject.toml` - Modern Python project config

## 🚀 Quick Commands

### First Time Setup
```bash
cd quatricmorph

# Verify everything works
cargo check --all
cd apps/web && npm install && npm run dev
```

### Development
```bash
# Rust backend
cargo build -p q-cli
cargo run -p q-cli -- --help
cargo run -p q-daemon

# Web frontend
cd apps/web
npm run dev  # http://localhost:3000

# All tests
cargo test --all
```

### Production
```bash
cargo build --release
cd apps/web && npm run build
```

## 📊 Project Statistics

| Category | Count |
|----------|-------|
| Rust Crates | 16 |
| GPU Implementations | 3 (WGSL, CUDA, Metal) |
| Model Architectures | 5 |
| Data Schemas | 4 |
| Documentation Files | 5 |
| Total Directories | 604 |
| Total Files | 4,412 |
| Source Code Files | 1,527 |

## ✅ Validation Status

- ✓ Rust workspace compiles with `cargo check --all`
- ✓ Web app configured and ready to run
- ✓ All 16 crates have proper structure
- ✓ Python package initialized
- ✓ GPU implementations templated
- ✓ Architecture configurations complete
- ✓ Data schemas defined
- ✓ Documentation complete

## 📝 Next Steps

1. **Understand the Architecture**
   - Read [SETUP_COMPLETE.md](./SETUP_COMPLETE.md)
   - Review [README.md](./README.md)

2. **Get It Running**
   - Follow [QUICK_START.md](./QUICK_START.md)
   - Start dev servers

3. **Start Development**
   - Read [DEVELOPMENT.md](./DEVELOPMENT.md)
   - Begin implementing crates

4. **Build Features**
   - Implement core tensor runtime
   - Add model loading
   - Build visualization UI
   - Connect components

## 🔗 File Locations

| Purpose | Location |
|---------|----------|
| Build config | `Cargo.toml` |
| CLI entry | `crates/q-cli/src/main.rs` |
| Daemon entry | `crates/q-daemon/src/main.rs` |
| Web entry | `apps/web/src/main.tsx` |
| Architecture config | `architectures/*/config.toml` |
| Schemas | `schemas/*/schema.json` |
| GPU shaders | `gpu/{wgsl,cuda,metal}/*` |

## 🛠️ Tools & Technologies

- **Language:** Rust 1.70+, TypeScript 5.0+, Python 3.9+
- **Framework:** React 18.2, Vite 4.0
- **Async Runtime:** Tokio
- **CLI:** Clap 4.0
- **GPU:** WGPU 0.19 (WebGPU), CUDA, Metal
- **Serialization:** Serde, SafeTensors
- **3D Graphics:** glTF, Three.js

## 📌 Important Notes

### Directory Structure
```
quatricmorph/
├── Cargo.toml              ← Root workspace
├── README.md               ← Start here
├── QUICK_START.md          ← How to build & run
├── DEVELOPMENT.md          ← Development guide
├── SETUP_COMPLETE.md       ← Validation results
├── GENERATED_STRUCTURE.md  ← Detailed structure
├── crates/                 ← Rust crates (16)
├── apps/web/               ← React app
├── gpu/                    ← GPU shaders
├── architectures/          ← Model configs
├── schemas/                ← Data formats
├── python/                 ← Python bindings
├── fixtures/               ← Test data
└── docs/                   ← Additional docs
```

### Building
- **Rust:** `cargo build --release`
- **Web:** `cd apps/web && npm run build`
- **Python:** `cd python && pip install -e .`

### Testing
- **Rust:** `cargo test --all`
- **Web:** `cd apps/web && npm test`

## 🤝 Contributing

1. Create a new feature branch
2. Make changes in appropriate crate
3. Run `cargo fmt && cargo clippy && cargo test`
4. Commit with meaningful message
5. Submit PR

## 📞 Questions?

- Check [DEVELOPMENT.md](./DEVELOPMENT.md) for workflows
- Review crate READMEs for specific modules
- See [QUICK_START.md](./QUICK_START.md) for common commands

---

**Project Status:** ✓ Ready for Development

**Last Updated:** 2026-08-03

**Next Steps:** Begin with [QUICK_START.md](./QUICK_START.md)
