# Quatricmorph

A modular system for visualization and analysis of neural network architectures.

## Project Structure

### Rust Crates (`crates/`)

- **q-source**: Source code parsing and management
- **q-safetensors**: SafeTensors format support
- **q-architecture**: Architecture definitions and utilities
- **q-nsir**: Neural Source Intermediate Representation
- **q-catalog**: Model catalog and registry
- **q-tensor-runtime**: Tensor computation runtime
- **q-statistics**: Statistical analysis tools
- **q-weightql**: Weight query language implementation
- **q-expression**: Expression parsing and evaluation
- **q-tiles**: Tile-based data structure
- **q-gltf**: glTF model support
- **q-tileset**: Tileset management
- **q-cache**: Caching layer
- **q-gpu**: GPU acceleration support (WGPU, CUDA, Metal)
- **q-daemon**: Background daemon service
- **q-cli**: Command-line interface

### GPU Support (`gpu/`)

- **wgsl/**: WebGPU shaders
- **cuda/**: CUDA kernel implementations
- **metal/**: Metal shader implementations

### Applications (`apps/`)

- **web/**: Web-based visualization and viewer (React + TypeScript)
- **desktop/**: Desktop application

### Python Bindings (`python/`)

- Python package for FFI to Rust crates

### Architectures (`architectures/`)

Model architecture definitions for:
- Llama
- Qwen
- Kimi
- DeepSeek
- Generic/Custom architectures

### Schemas (`schemas/`)

Schema definitions for:
- NSIR (Neural Source Intermediate Representation)
- QTile (Quantized Tile format)
- WeightQL (Weight Query Language)
- Visualization data

### Fixtures (`fixtures/`)

Test data and example models

### Documentation (`docs/`)

Architecture decisions, requirements, and roadmap
