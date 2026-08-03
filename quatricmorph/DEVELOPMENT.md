# Quatricmorph Development Guide

## Development Workflow

### 1. Environment Setup

```bash
# Navigate to project
cd quatricmorph

# Check Rust version (needs 1.70+)
rustc --version

# Check Node version (needs 18+)
node --version

# Build all crates
cargo build
```

### 2. Working with Rust Crates

#### Adding a new dependency
Edit the relevant `crates/q-*/Cargo.toml`:

```toml
[dependencies]
new_crate = "1.0"
```

Then run:
```bash
cargo check -p q-cratename
```

#### Creating a new module
1. Create file: `crates/q-cratename/src/mymodule.rs`
2. Add to lib.rs: `pub mod mymodule;`
3. Export what's needed: `pub use mymodule::MyStruct;`

#### Building for release
```bash
cargo build --release
# Binaries in: target/release/
```

### 3. Working with the Web App

#### Development server
```bash
cd apps/web
npm run dev
# Opens http://localhost:3000 with hot reload
```

#### Building for production
```bash
cd apps/web
npm run build
# Output in: dist/
```

#### Adding new dependencies
```bash
cd apps/web
npm install new-package
npm run dev  # Reload
```

#### Component structure
```
src/
├── App.tsx              # Main component
├── components/          # Reusable components
├── pages/              # Page components
├── hooks/              # Custom hooks
├── utils/              # Helper functions
└── styles/             # Global styles
```

### 4. Testing

#### Rust unit tests
```bash
# In any crate
cargo test -p q-cratename

# All tests
cargo test --all
```

Add tests in crate's src:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Test code
    }
}
```

#### Web tests
```bash
cd apps/web
npm test
```

### 5. Code Organization Principles

#### Crate Dependencies
- `q-cli` → uses all other crates as needed
- `q-daemon` → core runtime services
- `q-tensor-runtime` → core computation
- Avoid circular dependencies

#### Module Organization
Each crate should follow:
```
src/
├── lib.rs          # Main exports
├── core/           # Core types
├── ops/            # Operations
└── tests/          # Tests
```

#### Error Handling
Use `thiserror` for custom errors:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
}
```

### 6. Git Workflow

#### Before committing
```bash
# Format code
cargo fmt --all

# Lint
cargo clippy --all

# Test
cargo test --all
```

#### Commit message format
```
feat: add new feature
fix: resolve bug
docs: update documentation
refactor: restructure code
```

### 7. Build Commands Reference

| Command | Purpose |
|---------|---------|
| `cargo check --all` | Fast validation |
| `cargo build --release` | Optimized build |
| `cargo test --all` | Run tests |
| `cargo fmt --all` | Format code |
| `cargo clippy --all` | Lint |
| `cargo doc --open` | Generate docs |
| `npm run dev` | Start web server |
| `npm run build` | Build web app |

### 8. Common Tasks

#### Add a new crate
1. Create: `mkdir crates/q-newcrate`
2. Add Cargo.toml (copy from existing)
3. Create src/lib.rs
4. Add to root Cargo.toml workspace members
5. Run: `cargo check --all`

#### Add inter-crate dependency
In dependent crate's Cargo.toml:
```toml
[dependencies]
q-other = { path = "../q-other", version = "0.1" }
```

#### Create a module
1. Create file: `src/mymodule.rs`
2. In lib.rs: `pub mod mymodule;`
3. Export: `pub use mymodule::{MyStruct, my_function};`

#### Run specific crate
```bash
cargo run -p q-cli -- arg1 arg2
cargo run -p q-daemon
```

### 9. Debugging

#### Rust debugging
```bash
# Run with debug output
RUST_LOG=debug cargo run -p q-cli

# With backtrace
RUST_BACKTRACE=1 cargo run -p q-cli
```

#### Web debugging
- Open DevTools (F12)
- Check Console for errors
- Use breakpoints in debugger
- Hot reload enabled by default

### 10. Documentation

#### Generate Rust docs
```bash
cargo doc --open
```

#### Add doc comments
```rust
/// Brief description
///
/// Longer description with examples.
///
/// # Examples
/// ```
/// let result = my_function(42);
/// ```
pub fn my_function(x: i32) -> i32 {
    x * 2
}
```

## Performance Considerations

### Rust
- Profile with: `cargo flamegraph -p q-cratename`
- Use release builds for benchmarks
- Avoid cloning in hot paths
- Consider using references

### Web
- Use React.memo for expensive components
- Lazy load components: `React.lazy(() => import('./Component'))`
- Monitor bundle size: `npm run build -- --analyze`
- Use Web Workers for heavy computation

## Common Issues & Solutions

### "error: could not compile `q-*`"
1. Run `cargo check --all` for detailed errors
2. Check for missing dependencies
3. Ensure no circular imports

### "Cannot find module"
1. Verify module is declared in lib.rs
2. Check file path matches module name
3. Run `cargo check` to validate

### Web app slow
1. Check Network tab (DevTools)
2. Profile with Chrome DevTools Performance tab
3. Check for unnecessary re-renders with React DevTools

### npm install fails
1. Clear cache: `npm cache clean --force`
2. Delete node_modules: `rm -rf node_modules package-lock.json`
3. Reinstall: `npm install`

## IDE Setup

### VS Code Extensions
- rust-analyzer (for Rust)
- Better TOML
- Thunder Client (for API testing)
- ES7+ React/Redux/React-Native snippets
- TypeScript Vue Plugin

### VS Code Settings
```json
{
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer",
    "editor.formatOnSave": true
  },
  "[typescript]": {
    "editor.defaultFormatter": "esbenp.prettier-vscode"
  }
}
```

## Continuous Improvement

1. Monitor compilation times
2. Keep dependencies updated: `cargo outdated`
3. Regular security audits: `cargo audit`
4. Performance benchmarks: `cargo bench`
5. Code reviews before merge
6. Documentation updates alongside code

---

**Happy coding!**
