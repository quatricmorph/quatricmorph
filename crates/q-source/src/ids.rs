//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §5).
//!
//! Stable content-derived identifiers.
//!
//! `ModelId` and `TensorId` must be identical across reopen, across processes,
//! and across machines, because they are the join key between the catalog, the
//! tile plane, the cache key (§13.2), and every canonical address in a report
//! or annotation. They are therefore derived by hashing stable *content*, never
//! by a counter, a UUIDv4, or an insertion order.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Bumping this invalidates every previously persisted ID. It is part of the
/// hash input so that a scheme change cannot silently collide with old data.
pub const ID_SCHEME_VERSION: u8 = 1;

macro_rules! define_id {
    ($name:ident, $domain:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub [u8; 16]);

        impl $name {
            pub const DOMAIN: &'static str = $domain;

            pub fn from_bytes(b: [u8; 16]) -> Self {
                Self(b)
            }

            pub fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }

            /// Lowercase 32-character hex, the form persisted in the catalog.
            pub fn to_hex(&self) -> String {
                let mut s = String::with_capacity(32);
                for b in self.0 {
                    s.push_str(&format!("{b:02x}"));
                }
                s
            }

            pub fn from_hex(s: &str) -> Option<Self> {
                if s.len() != 32 {
                    return None;
                }
                let mut out = [0u8; 16];
                for i in 0..16 {
                    out[i] = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
                }
                Some(Self(out))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.to_hex())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_hex())
            }
        }
    };
}

define_id!(
    ModelId,
    "quatricmorph/model/v1",
    "Stable identifier for an imported checkpoint."
);
define_id!(
    TensorId,
    "quatricmorph/tensor/v1",
    "Stable identifier for one tensor within one model."
);

fn digest16(domain: &str, parts: &[&[u8]]) -> [u8; 16] {
    let mut h = blake3::Hasher::new();
    h.update(&[ID_SCHEME_VERSION]);
    h.update(domain.as_bytes());
    h.update(&[0]);
    for p in parts {
        // Length-prefix each part so that ("ab","c") and ("a","bc") differ.
        h.update(&(p.len() as u64).to_le_bytes());
        h.update(p);
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

impl ModelId {
    /// Derive from the identity of the source, not from where it happens to sit
    /// on this machine.
    ///
    /// * `source_key` — a stable logical name (`"hf:meta-llama/Llama-3-8B"`, or
    ///   a directory name for local imports);
    /// * `revision` — the revision/commit if known, `""` otherwise;
    /// * `content_fingerprint` — a fingerprint of the artifact set (see
    ///   [`content_fingerprint`]). Including it means two checkouts of the same
    ///   logical model with different bytes get different IDs.
    pub fn derive(source_key: &str, revision: &str, content_fingerprint: &str) -> Self {
        Self(digest16(
            Self::DOMAIN,
            &[
                source_key.as_bytes(),
                revision.as_bytes(),
                content_fingerprint.as_bytes(),
            ],
        ))
    }
}

impl TensorId {
    /// Derive from the owning model and the tensor's *raw* name.
    ///
    /// The raw name is used rather than the canonical name because canonical
    /// naming depends on which architecture resolver ran, and a resolver
    /// improvement must not change the identity of already-catalogued tensors.
    pub fn derive(model: ModelId, raw_name: &str) -> Self {
        Self(digest16(Self::DOMAIN, &[model.as_bytes(), raw_name.as_bytes()]))
    }
}

/// Fingerprint an artifact set without reading its payload.
///
/// Deliberately cheap: it hashes the *manifest* (file names and lengths), not
/// the weights. Hashing 600 GB of weights to open a model would violate the
/// bounded-memory / bounded-IO contract. The trade-off is stated plainly: this
/// detects renamed, added, removed, or resized shards, and does **not** detect
/// an in-place edit that preserves every file length.
pub fn content_fingerprint(files: &[(String, u64)]) -> String {
    let mut sorted: Vec<&(String, u64)> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut h = blake3::Hasher::new();
    h.update(b"quatricmorph/manifest-fingerprint/v1");
    for (name, len) in sorted {
        h.update(&(name.len() as u64).to_le_bytes());
        h.update(name.as_bytes());
        h.update(&len.to_le_bytes());
    }
    h.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_across_calls() {
        let a = ModelId::derive("local:tiny", "", "fp");
        let b = ModelId::derive("local:tiny", "", "fp");
        assert_eq!(a, b);
    }

    #[test]
    fn different_revisions_give_different_model_ids() {
        assert_ne!(
            ModelId::derive("hf:org/m", "r1", "fp"),
            ModelId::derive("hf:org/m", "r2", "fp")
        );
    }

    #[test]
    fn tensor_ids_are_namespaced_by_model() {
        let m1 = ModelId::derive("a", "", "fp");
        let m2 = ModelId::derive("b", "", "fp");
        let name = "model.layers.10.self_attn.q_proj.weight";
        assert_ne!(TensorId::derive(m1, name), TensorId::derive(m2, name));
        assert_eq!(TensorId::derive(m1, name), TensorId::derive(m1, name));
    }

    #[test]
    fn length_prefixing_prevents_concatenation_collisions() {
        let m = ModelId::derive("x", "", "fp");
        assert_ne!(TensorId::derive(m, "ab.c"), TensorId::derive(m, "a.bc"));
    }

    #[test]
    fn hex_round_trip() {
        let id = TensorId::derive(ModelId::derive("m", "", "f"), "t");
        assert_eq!(TensorId::from_hex(&id.to_hex()).unwrap(), id);
        assert_eq!(id.to_hex().len(), 32);
        assert!(TensorId::from_hex("nope").is_none());
    }

    #[test]
    fn fingerprint_is_order_independent_but_content_sensitive() {
        let a = content_fingerprint(&[("b".into(), 2), ("a".into(), 1)]);
        let b = content_fingerprint(&[("a".into(), 1), ("b".into(), 2)]);
        assert_eq!(a, b);
        let c = content_fingerprint(&[("a".into(), 1), ("b".into(), 3)]);
        assert_ne!(a, c);
    }
}
