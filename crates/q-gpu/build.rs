//! Build script for `q-gpu`.
//!
//! Data plane: build tooling for the **Tensor Tile Plane** (ARCHITECTURE.md
//! §2.1, §12.3). Requirement: `GPU-003`.
//!
//! # It does nothing at all unless the `metal` feature is on
//!
//! The feature is off by default, and with it off this script emits three
//! `rerun-if-changed` lines and returns. `cargo build --workspace` on a machine
//! with no Metal toolchain must succeed — that is acceptance criterion 1 of
//! `QM-0126` and it is what keeps CI green — so nothing below runs
//! speculatively and nothing is probed "just in case".
//!
//! # With the feature on it either produces the real artifacts or fails loudly
//!
//! There is deliberately **no fallback**. If `xcrun metal` is absent, or the
//! shader does not compile, or the Objective-C shim does not compile, this
//! script panics with a message naming the file — it never quietly disables the
//! backend and ships a stub that would report itself as a Metal GPU. That is
//! `QM-0126`'s Error Handling row *"Shader compilation fails at build time →
//! build error naming the shader — never a silent fallback that ships a stub"*,
//! and it is the same rule as `gpu/cuda/*.cu` being honestly uncompiled rather
//! than dishonestly "supported".
//!
//! A toolchain that is missing **while the feature is explicitly on** is a
//! build failure. A *device* that is missing at run time is not: that is
//! `MetalBackend::new` returning `None`. The two are different conditions and
//! this script only ever decides the first.
//!
//! # What it produces
//!
//! | artifact | from | consumed by |
//! | --- | --- | --- |
//! | `$OUT_DIR/paired_reduction.metallib` | `gpu/metal/paired_reduction.metal` | `include_bytes!` in `src/metal.rs` |
//! | `$OUT_DIR/libqm_metal_shim.a` | `gpu/metal/qm_metal_shim.m` | statically linked |
//!
//! The metallib is embedded rather than loaded from a path, so a relocated or
//! deleted artifact is a compile error instead of a runtime surprise.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

const SHADER: &str = "gpu/metal/paired_reduction.metal";
const SHIM: &str = "gpu/metal/qm_metal_shim.m";

fn main() {
    let repo_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root is two levels above crates/q-gpu");

    println!("cargo:rerun-if-changed=build.rs");
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join(SHADER).display()
    );
    println!("cargo:rerun-if-changed={}", repo_root.join(SHIM).display());

    // `CARGO_FEATURE_*`, not `cfg!(feature = ...)`: a build script is compiled
    // for the host without the crate's own features, so `cfg!` here would read
    // `false` no matter what the caller asked for.
    if env::var_os("CARGO_FEATURE_METAL").is_none() {
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "macos" {
        panic!(
            "q-gpu's `metal` feature was enabled for target_os = \"{target_os}\", but Metal \
             exists only on Apple platforms. Build without `--features metal`; the default \
             build uses q_gpu::CpuBackend and needs no Metal toolchain."
        );
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    build_metallib(&repo_root, &out_dir);
    build_shim(&repo_root, &out_dir);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=qm_metal_shim");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=Foundation");
}

/// Compile the shader to a metallib, or fail naming the shader.
fn build_metallib(repo_root: &Path, out_dir: &Path) {
    let shader = repo_root.join(SHADER);
    let metallib = out_dir.join("paired_reduction.metallib");
    let output = Command::new("xcrun")
        .args(["-sdk", "macosx", "metal", "-O2", "-std=metal3.0", "-o"])
        .arg(&metallib)
        .arg(&shader)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "q-gpu `metal` feature: could not run `xcrun -sdk macosx metal` to compile \
                 {SHADER}: {e}. Install the Xcode command line tools, or build without \
                 `--features metal`."
            )
        });
    if !output.status.success() {
        panic!(
            "q-gpu `metal` feature: {SHADER} failed to compile ({}).\n--- stderr ---\n{}\n\
             This is a build error on purpose: a shader that does not compile must never fall \
             back to a stub that still reports itself as a Metal backend.",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Compile and archive the Objective-C shim, or fail naming it.
fn build_shim(repo_root: &Path, out_dir: &Path) {
    let shim = repo_root.join(SHIM);
    let object = out_dir.join("qm_metal_shim.o");
    let archive = out_dir.join("libqm_metal_shim.a");

    let compile = Command::new("xcrun")
        .args([
            "clang",
            "-x",
            "objective-c",
            "-fobjc-arc",
            "-fmodules",
            "-O2",
            "-Wall",
            "-Werror",
            "-c",
        ])
        .arg(&shim)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "q-gpu `metal` feature: could not run `xcrun clang` to compile {SHIM}: {e}. \
                 Install the Xcode command line tools, or build without `--features metal`."
            )
        });
    if !compile.status.success() {
        panic!(
            "q-gpu `metal` feature: {SHIM} failed to compile ({}).\n--- stderr ---\n{}",
            compile.status,
            String::from_utf8_lossy(&compile.stderr)
        );
    }

    // Rebuild the archive from scratch: `ar r` into a stale archive would keep
    // an old object file alongside the new one.
    let _ = std::fs::remove_file(&archive);
    let ar = Command::new("xcrun")
        .arg("ar")
        .arg("rcs")
        .arg(&archive)
        .arg(&object)
        .output()
        .unwrap_or_else(|e| panic!("q-gpu `metal` feature: could not run `ar` for {SHIM}: {e}"));
    if !ar.status.success() {
        panic!(
            "q-gpu `metal` feature: archiving {SHIM} failed ({}).\n--- stderr ---\n{}",
            ar.status,
            String::from_utf8_lossy(&ar.stderr)
        );
    }
}
