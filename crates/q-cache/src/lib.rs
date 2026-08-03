//! # q-cache — Tensor Tile Plane / Metadata Plane
//!
//! Data plane: caches results from the **Tensor Tile Plane** and the
//! **Metadata Plane** (ARCHITECTURE.md §2.1, §13).
//!
//! ## The five levels (§13.1)
//!
//! ```text
//! L0 — GPU resident        visible tiles and selected tensors   (not built: CACHE-005)
//! L1 — Process memory      decoded qtiles and hot metadata      IMPLEMENTED
//! L2 — Local NVMe          content-addressed tile/analysis cache IMPLEMENTED
//! L3 — Browser             Cache Storage / IndexedDB            (trait stub: CACHE-006)
//! L4 — Remote object store published tiles and shared summaries (trait stub: CACHE-007)
//! ```
//!
//! L1 and L2 are real, with size limits, eviction, and reuse across process
//! restarts. L3 and L4 are [`CacheTier`] implementations that return
//! [`QError::NotImplemented`] — they cannot silently miss and pretend to be
//! empty, because a silent miss is indistinguishable from a working cache that
//! never hits.
//!
//! ## The cache key (§13.2)
//!
//! ```text
//! hash(source_model_hash, tensor_id, logical_slice, lod,
//!      summary_algorithm, algorithm_version, visualization_encoding)
//! ```
//!
//! Note what is *absent*: the colour palette. ARCHITECTURE.md §13.2 says colour
//! need not be part of the key when it is computed in the shader, and it is —
//! so recolouring never invalidates a cached tile.

use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Default L1 capacity in entries. Named, not magic.
pub const DEFAULT_L1_ENTRIES: usize = 512;
/// Default L2 ceiling: 8 GiB of local NVMe.
pub const DEFAULT_L2_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// The components of the §13.2 cache key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    /// Fingerprint of the source checkpoint.
    pub source_model_hash: String,
    pub tensor_id: String,
    /// Canonical spelling of the selected region, e.g. `[100:104,40:44]`.
    pub logical_slice: String,
    pub lod: u8,
    /// e.g. `"statistics"`, `"quantized_tile"`.
    pub summary_algorithm: String,
    pub algorithm_version: u32,
    /// e.g. `"raw_f32"`, `"quantized_i16"`.
    pub visualization_encoding: String,
}

impl CacheKey {
    /// Content address: a stable hex digest of every component.
    ///
    /// Length-prefixing each field means `("ab","c")` and `("a","bc")` cannot
    /// collide — a real hazard when tensor IDs and slice strings are adjacent.
    pub fn digest(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"quatricmorph/cache-key/v1");
        for part in [
            self.source_model_hash.as_bytes(),
            self.tensor_id.as_bytes(),
            self.logical_slice.as_bytes(),
            self.summary_algorithm.as_bytes(),
            self.visualization_encoding.as_bytes(),
        ] {
            h.update(&(part.len() as u64).to_le_bytes());
            h.update(part);
        }
        h.update(&[self.lod]);
        h.update(&self.algorithm_version.to_le_bytes());
        h.finalize().to_hex().to_string()
    }

    /// Filesystem-safe relative path, sharded by digest prefix so a single
    /// directory never accumulates millions of entries.
    pub fn relative_path(&self) -> PathBuf {
        let d = self.digest();
        PathBuf::from(&d[0..2])
            .join(&d[2..4])
            .join(format!("{d}.qcache"))
    }
}

/// A cache level.
///
/// L3 and L4 implement this and refuse. That is deliberate: a stub that
/// returned `Ok(None)` would look exactly like a cold cache forever.
pub trait CacheTier: Send + Sync {
    fn name(&self) -> &'static str;
    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>>;
    fn put(&self, key: &CacheKey, value: &[u8]) -> Result<()>;
    fn contains(&self, key: &CacheKey) -> Result<bool> {
        Ok(self.get(key)?.is_some())
    }
}

// --- L1: in-process LRU ------------------------------------------------------

/// L1 — process memory. Bounded by entry count and total bytes.
pub struct L1Cache {
    inner: Mutex<lru::LruCache<String, Vec<u8>>>,
    max_bytes: u64,
    bytes: Mutex<u64>,
    hits: Mutex<u64>,
    misses: Mutex<u64>,
}

impl L1Cache {
    pub fn new(max_entries: usize, max_bytes: u64) -> Self {
        Self {
            inner: Mutex::new(lru::LruCache::new(
                NonZeroUsize::new(max_entries.max(1)).expect("max(1) is non-zero"),
            )),
            max_bytes,
            bytes: Mutex::new(0),
            hits: Mutex::new(0),
            misses: Mutex::new(0),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_L1_ENTRIES, 256 * 1024 * 1024)
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn bytes(&self) -> u64 {
        *self.bytes.lock().unwrap()
    }

    /// `(hits, misses)` since construction.
    pub fn stats(&self) -> (u64, u64) {
        (*self.hits.lock().unwrap(), *self.misses.lock().unwrap())
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
        *self.bytes.lock().unwrap() = 0;
    }
}

impl CacheTier for L1Cache {
    fn name(&self) -> &'static str {
        "L1"
    }

    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        let mut lru = self.inner.lock().unwrap();
        match lru.get(&key.digest()) {
            Some(v) => {
                *self.hits.lock().unwrap() += 1;
                Ok(Some(v.clone()))
            }
            None => {
                *self.misses.lock().unwrap() += 1;
                Ok(None)
            }
        }
    }

    fn put(&self, key: &CacheKey, value: &[u8]) -> Result<()> {
        if value.len() as u64 > self.max_bytes {
            return Err(QError::BudgetExceeded {
                budget_name: "l1_entry",
                requested: value.len() as u64,
                limit: self.max_bytes,
            });
        }
        let mut lru = self.inner.lock().unwrap();
        let mut bytes = self.bytes.lock().unwrap();
        let digest = key.digest();
        if let Some(old) = lru.pop(&digest) {
            *bytes -= old.len() as u64;
        }
        // Evict least-recently-used entries until the new one fits.
        while *bytes + value.len() as u64 > self.max_bytes {
            match lru.pop_lru() {
                Some((_, evicted)) => *bytes -= evicted.len() as u64,
                None => break,
            }
        }
        if let Some((_, evicted)) = lru.push(digest, value.to_vec()) {
            *bytes -= evicted.len() as u64;
        }
        *bytes += value.len() as u64;
        Ok(())
    }
}

// --- L2: content-addressed local disk ----------------------------------------

/// L2 — content-addressed local NVMe cache.
///
/// Entries are files named by their key digest, so a cache directory can be
/// inspected, copied, or deleted with ordinary tools, and survives process
/// restarts by construction. Eviction is by least-recently-modified once the
/// directory exceeds `max_bytes`.
pub struct L2Cache {
    root: PathBuf,
    max_bytes: u64,
}

impl L2Cache {
    pub fn open(root: impl AsRef<Path>, max_bytes: u64) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root).map_err(|e| QError::Io {
            path: root.clone(),
            source: e,
        })?;
        Ok(Self { root, max_bytes })
    }

    pub fn open_default(root: impl AsRef<Path>) -> Result<Self> {
        Self::open(root, DEFAULT_L2_MAX_BYTES)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, key: &CacheKey) -> PathBuf {
        self.root.join(key.relative_path())
    }

    /// Every cache file, with its size and modification time.
    fn entries(&self) -> Result<Vec<(PathBuf, u64, std::time::SystemTime)>> {
        let mut out = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let read = match std::fs::read_dir(&dir) {
                Ok(r) => r,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    return Err(QError::Io {
                        path: dir,
                        source: e,
                    })
                }
            };
            for entry in read {
                let entry = entry.map_err(|e| QError::Io {
                    path: dir.clone(),
                    source: e,
                })?;
                let meta = entry.metadata().map_err(|e| QError::Io {
                    path: entry.path(),
                    source: e,
                })?;
                if meta.is_dir() {
                    stack.push(entry.path());
                } else if entry.path().extension().is_some_and(|e| e == "qcache") {
                    let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                    out.push((entry.path(), meta.len(), modified));
                }
            }
        }
        Ok(out)
    }

    pub fn total_bytes(&self) -> Result<u64> {
        Ok(self.entries()?.iter().map(|(_, len, _)| len).sum())
    }

    pub fn entry_count(&self) -> Result<usize> {
        Ok(self.entries()?.len())
    }

    /// Evict least-recently-modified entries until the cache fits.
    pub fn evict_to_fit(&self, incoming_bytes: u64) -> Result<usize> {
        let mut entries = self.entries()?;
        let mut total: u64 = entries.iter().map(|(_, len, _)| len).sum();
        if total + incoming_bytes <= self.max_bytes {
            return Ok(0);
        }
        entries.sort_by_key(|(_, _, modified)| *modified);
        let mut evicted = 0usize;
        for (path, len, _) in entries {
            if total + incoming_bytes <= self.max_bytes {
                break;
            }
            std::fs::remove_file(&path).map_err(|e| QError::Io { path, source: e })?;
            total = total.saturating_sub(len);
            evicted += 1;
        }
        Ok(evicted)
    }
}

impl CacheTier for L2Cache {
    fn name(&self) -> &'static str {
        "L2"
    }

    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        let path = self.path_for(key);
        match std::fs::read(&path) {
            Ok(v) => Ok(Some(v)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(QError::Io { path, source: e }),
        }
    }

    fn put(&self, key: &CacheKey, value: &[u8]) -> Result<()> {
        if value.len() as u64 > self.max_bytes {
            return Err(QError::BudgetExceeded {
                budget_name: "l2_entry",
                requested: value.len() as u64,
                limit: self.max_bytes,
            });
        }
        self.evict_to_fit(value.len() as u64)?;
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| QError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        // Write-then-rename: a reader never observes a half-written entry.
        let tmp = path.with_extension("qcache.tmp");
        std::fs::write(&tmp, value).map_err(|e| QError::Io {
            path: tmp.clone(),
            source: e,
        })?;
        std::fs::rename(&tmp, &path).map_err(|e| QError::Io { path, source: e })?;
        Ok(())
    }
}

// --- L3 / L4: declared, not built --------------------------------------------

/// L3 — browser Cache Storage / IndexedDB. **Not built** (`CACHE-006`).
pub struct L3BrowserCache;

impl CacheTier for L3BrowserCache {
    fn name(&self) -> &'static str {
        "L3"
    }

    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        Err(QError::not_implemented(
            "CACHE-006",
            format!(
                "the L3 browser cache (Cache Storage / IndexedDB) is not built in this pass; \
                 key {} was not looked up. See ARCHITECTURE.md §13.1.",
                key.digest()
            ),
        ))
    }

    fn put(&self, _key: &CacheKey, _value: &[u8]) -> Result<()> {
        Err(QError::not_implemented(
            "CACHE-006",
            "the L3 browser cache is not built in this pass. See ARCHITECTURE.md §13.1.",
        ))
    }
}

/// L4 — remote object storage and CDN. **Not built** (`CACHE-007`).
pub struct L4RemoteCache;

impl CacheTier for L4RemoteCache {
    fn name(&self) -> &'static str {
        "L4"
    }

    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        Err(QError::not_implemented(
            "CACHE-007",
            format!(
                "the L4 remote object-storage cache is not built in this pass; key {} was not \
                 looked up. See ARCHITECTURE.md §13.1 and §17 Phase 6.",
                key.digest()
            ),
        ))
    }

    fn put(&self, _key: &CacheKey, _value: &[u8]) -> Result<()> {
        Err(QError::not_implemented(
            "CACHE-007",
            "the L4 remote cache is not built in this pass. See ARCHITECTURE.md §13.1.",
        ))
    }
}

// --- the layered cache -------------------------------------------------------

/// L1 in front of L2, with promotion on an L2 hit.
pub struct LayeredCache {
    l1: L1Cache,
    l2: Option<L2Cache>,
}

/// Which level answered a lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitLevel {
    L1,
    L2,
    Miss,
}

impl LayeredCache {
    pub fn new(l1: L1Cache, l2: Option<L2Cache>) -> Self {
        Self { l1, l2 }
    }

    pub fn memory_only() -> Self {
        Self::new(L1Cache::with_defaults(), None)
    }

    pub fn l1(&self) -> &L1Cache {
        &self.l1
    }

    pub fn l2(&self) -> Option<&L2Cache> {
        self.l2.as_ref()
    }

    /// Look up, reporting which level answered.
    pub fn get_with_level(&self, key: &CacheKey) -> Result<(Option<Vec<u8>>, HitLevel)> {
        if let Some(v) = self.l1.get(key)? {
            return Ok((Some(v), HitLevel::L1));
        }
        if let Some(l2) = &self.l2 {
            if let Some(v) = l2.get(key)? {
                // Promote so the next read is an L1 hit.
                self.l1.put(key, &v)?;
                return Ok((Some(v), HitLevel::L2));
            }
        }
        Ok((None, HitLevel::Miss))
    }

    pub fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        Ok(self.get_with_level(key)?.0)
    }

    /// Write through both levels.
    pub fn put(&self, key: &CacheKey, value: &[u8]) -> Result<()> {
        self.l1.put(key, value)?;
        if let Some(l2) = &self.l2 {
            l2.put(key, value)?;
        }
        Ok(())
    }
}

/// A trivial in-memory tier used to prove [`CacheTier`] is implementable
/// without touching disk. Not part of the §13.1 ladder.
#[derive(Default)]
pub struct MemoryTier(Mutex<HashMap<String, Vec<u8>>>);

impl CacheTier for MemoryTier {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn get(&self, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        Ok(self.0.lock().unwrap().get(&key.digest()).cloned())
    }

    fn put(&self, key: &CacheKey, value: &[u8]) -> Result<()> {
        self.0.lock().unwrap().insert(key.digest(), value.to_vec());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(slice: &str) -> CacheKey {
        CacheKey {
            source_model_hash: "b3:abc".into(),
            tensor_id: "0123456789abcdef0123456789abcdef".into(),
            logical_slice: slice.into(),
            lod: 4,
            summary_algorithm: "statistics".into(),
            algorithm_version: 1,
            visualization_encoding: "raw_f32".into(),
        }
    }

    #[test]
    fn every_key_component_changes_the_digest() {
        let base = key("[0:4,0:4]");
        let d = base.digest();
        let mut k = base.clone();
        k.logical_slice = "[0:8,0:8]".into();
        assert_ne!(k.digest(), d);
        let mut k = base.clone();
        k.lod = 3;
        assert_ne!(k.digest(), d);
        let mut k = base.clone();
        k.algorithm_version = 2;
        assert_ne!(k.digest(), d);
        let mut k = base.clone();
        k.visualization_encoding = "quantized_i16".into();
        assert_ne!(k.digest(), d);
        let mut k = base.clone();
        k.source_model_hash = "b3:def".into();
        assert_ne!(k.digest(), d);
        let mut k = base.clone();
        k.summary_algorithm = "histogram".into();
        assert_ne!(k.digest(), d);
        // ...and the digest is stable for an unchanged key.
        assert_eq!(base.digest(), key("[0:4,0:4]").digest());
    }

    #[test]
    fn length_prefixing_prevents_field_boundary_collisions() {
        let mut a = key("[0:4,0:4]");
        a.tensor_id = "ab".into();
        a.logical_slice = "c".into();
        let mut b = a.clone();
        b.tensor_id = "a".into();
        b.logical_slice = "bc".into();
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn l1_write_read_and_stats() {
        let c = L1Cache::new(8, 1024);
        assert!(c.is_empty());
        assert!(c.get(&key("a")).unwrap().is_none());
        c.put(&key("a"), b"hello").unwrap();
        assert_eq!(c.get(&key("a")).unwrap().unwrap(), b"hello");
        assert_eq!(c.len(), 1);
        assert_eq!(c.bytes(), 5);
        let (hits, misses) = c.stats();
        assert_eq!((hits, misses), (1, 1));
    }

    #[test]
    fn l1_evicts_by_entry_count() {
        let c = L1Cache::new(2, 1_000_000);
        c.put(&key("a"), b"1").unwrap();
        c.put(&key("b"), b"2").unwrap();
        c.put(&key("c"), b"3").unwrap();
        assert_eq!(c.len(), 2);
        // `a` was least-recently-used.
        assert!(c.get(&key("a")).unwrap().is_none());
        assert!(c.get(&key("c")).unwrap().is_some());
    }

    #[test]
    fn l1_evicts_by_byte_budget() {
        let c = L1Cache::new(100, 10);
        c.put(&key("a"), &[0u8; 6]).unwrap();
        c.put(&key("b"), &[0u8; 6]).unwrap();
        assert!(c.bytes() <= 10);
        assert!(c.get(&key("a")).unwrap().is_none());
        assert!(c.get(&key("b")).unwrap().is_some());
        // An entry larger than the whole budget is refused, not silently dropped.
        assert!(matches!(
            c.put(&key("huge"), &[0u8; 64]),
            Err(QError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn l1_overwrite_does_not_double_count_bytes() {
        let c = L1Cache::new(8, 1024);
        c.put(&key("a"), &[0u8; 100]).unwrap();
        c.put(&key("a"), &[0u8; 10]).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c.bytes(), 10);
    }

    #[test]
    fn l2_write_read_and_content_addressed_layout() {
        let dir = tempfile::tempdir().unwrap();
        let c = L2Cache::open(dir.path(), 1_000_000).unwrap();
        assert!(c.get(&key("a")).unwrap().is_none());
        c.put(&key("a"), b"payload").unwrap();
        assert_eq!(c.get(&key("a")).unwrap().unwrap(), b"payload");
        assert_eq!(c.entry_count().unwrap(), 1);
        assert_eq!(c.total_bytes().unwrap(), 7);
        // Sharded by digest prefix.
        let rel = key("a").relative_path();
        assert!(dir.path().join(&rel).exists());
        assert_eq!(rel.components().count(), 3);
        // No temp files left behind.
        assert!(!dir.path().join(rel.with_extension("qcache.tmp")).exists());
    }

    #[test]
    fn l2_is_reused_after_reopen() {
        // ARCHITECTURE.md §18 AC-008: the cache is reused after reopening.
        let dir = tempfile::tempdir().unwrap();
        {
            let c = L2Cache::open(dir.path(), 1_000_000).unwrap();
            c.put(&key("persisted"), b"survives").unwrap();
        }
        let reopened = L2Cache::open(dir.path(), 1_000_000).unwrap();
        assert_eq!(
            reopened.get(&key("persisted")).unwrap().unwrap(),
            b"survives"
        );
        assert_eq!(reopened.entry_count().unwrap(), 1);
    }

    #[test]
    fn l2_evicts_to_stay_under_its_budget() {
        let dir = tempfile::tempdir().unwrap();
        let c = L2Cache::open(dir.path(), 16).unwrap();
        c.put(&key("a"), &[1u8; 8]).unwrap();
        // Distinct mtimes so eviction order is deterministic on coarse-grained
        // filesystems.
        std::thread::sleep(std::time::Duration::from_millis(20));
        c.put(&key("b"), &[2u8; 8]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        c.put(&key("c"), &[3u8; 8]).unwrap();
        assert!(c.total_bytes().unwrap() <= 16);
        assert!(
            c.get(&key("a")).unwrap().is_none(),
            "oldest should be evicted"
        );
        assert!(c.get(&key("c")).unwrap().is_some());
        assert!(matches!(
            c.put(&key("huge"), &[0u8; 64]),
            Err(QError::BudgetExceeded { .. })
        ));
    }

    #[test]
    fn layered_cache_promotes_l2_hits_into_l1() {
        let dir = tempfile::tempdir().unwrap();
        let cache = LayeredCache::new(
            L1Cache::new(8, 1024),
            Some(L2Cache::open(dir.path(), 1_000_000).unwrap()),
        );
        assert_eq!(cache.get_with_level(&key("a")).unwrap().1, HitLevel::Miss);

        cache.put(&key("a"), b"v").unwrap();
        assert_eq!(cache.get_with_level(&key("a")).unwrap().1, HitLevel::L1);

        // Drop L1 and the value still comes back — from L2, then promoted.
        cache.l1().clear();
        assert_eq!(cache.get_with_level(&key("a")).unwrap().1, HitLevel::L2);
        assert_eq!(cache.get_with_level(&key("a")).unwrap().1, HitLevel::L1);
    }

    #[test]
    fn layered_cache_survives_reopen_of_its_l2() {
        let dir = tempfile::tempdir().unwrap();
        {
            let cache = LayeredCache::new(
                L1Cache::new(8, 1024),
                Some(L2Cache::open(dir.path(), 1_000_000).unwrap()),
            );
            cache.put(&key("session1"), b"result").unwrap();
        }
        let reopened = LayeredCache::new(
            L1Cache::new(8, 1024),
            Some(L2Cache::open(dir.path(), 1_000_000).unwrap()),
        );
        let (value, level) = reopened.get_with_level(&key("session1")).unwrap();
        assert_eq!(value.unwrap(), b"result");
        assert_eq!(level, HitLevel::L2);
    }

    #[test]
    fn l3_and_l4_refuse_rather_than_missing_silently() {
        // A stub that returned Ok(None) would be indistinguishable from a cache
        // that never hits, which is exactly the failure mode §20 forbids.
        let k = key("a");
        for tier in [
            Box::new(L3BrowserCache) as Box<dyn CacheTier>,
            Box::new(L4RemoteCache),
        ] {
            let err = tier.get(&k).unwrap_err();
            assert!(
                err.requirement_id().is_some(),
                "{} lacks a requirement ID",
                tier.name()
            );
            assert!(tier.put(&k, b"x").is_err());
        }
        assert_eq!(
            L3BrowserCache.get(&k).unwrap_err().requirement_id(),
            Some("CACHE-006")
        );
        assert_eq!(
            L4RemoteCache.get(&k).unwrap_err().requirement_id(),
            Some("CACHE-007")
        );
    }

    #[test]
    fn the_tier_trait_is_implementable_by_third_parties() {
        let t = MemoryTier::default();
        assert_eq!(t.name(), "memory");
        assert!(!t.contains(&key("a")).unwrap());
        t.put(&key("a"), b"x").unwrap();
        assert!(t.contains(&key("a")).unwrap());
    }
}
