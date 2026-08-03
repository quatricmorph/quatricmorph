//! Cross-crate integration tests for Quatricmorph.
//!
//! This package has no library surface of its own; everything lives in
//! `tests/tests/`. It exists as a workspace member so that
//! `cargo test --workspace` picks up integration tests that span several
//! crates — most importantly the Section 7 end-to-end vertical slice in
//! `tests/tests/end_to_end_scalar_slice.rs`.
