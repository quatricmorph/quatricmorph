//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §4.1).
//!
//! Memory-mapped local-disk [`ModelSource`].
//!
//! Reads go through `mmap`, so a range read costs the pages it touches and
//! nothing else — opening a 600 GB shard and reading four bytes from the middle
//! of it does not allocate 600 GB. Maps are cached per file behind an `Arc` so
//! repeated reads of the same shard do not re-map it.
//!
//! Security: [`LocalFsSource`] confines every access to its configured root
//! (`SEC-001`). A `uri` that escapes the root via `..`, an absolute path, or a
//! symlink is rejected with [`QError::PathOutsideRoot`].

use crate::error::{QError, Result};
use crate::manifest::{
    ArtifactKind, ByteStream, ModelManifest, ModelSource, SourceFile,
};
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A read-only window into a memory map.
struct MmapWindow {
    map: Arc<Mmap>,
    pos: usize,
    end: usize,
}

impl Read for MmapWindow {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = (self.end - self.pos).min(buf.len());
        buf[..n].copy_from_slice(&self.map[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// A checkpoint directory on local disk.
pub struct LocalFsSource {
    root: PathBuf,
    source_key: String,
    revision: String,
    maps: Mutex<HashMap<String, Arc<Mmap>>>,
}

impl LocalFsSource {
    /// Open `root` as a checkpoint directory.
    ///
    /// `source_key` defaults to `local:<directory name>`; pass an explicit one
    /// via [`LocalFsSource::with_source_key`] when the directory name is not a
    /// stable identity (a temp dir, for instance).
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let canonical = root.canonicalize().map_err(|e| QError::Io {
            path: root.to_path_buf(),
            source: e,
        })?;
        let key = canonical
            .file_name()
            .map(|s| format!("local:{}", s.to_string_lossy()))
            .unwrap_or_else(|| "local:unnamed".to_string());
        Ok(Self {
            root: canonical,
            source_key: key,
            revision: String::new(),
            maps: Mutex::new(HashMap::new()),
        })
    }

    pub fn with_source_key(mut self, key: impl Into<String>) -> Self {
        self.source_key = key.into();
        self
    }

    pub fn with_revision(mut self, revision: impl Into<String>) -> Self {
        self.revision = revision.into();
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve `uri` against the root, refusing anything that escapes it.
    ///
    /// The check is performed on the *canonicalized* path, so it also catches
    /// symlinks pointing outside the root.
    pub fn resolve(&self, uri: &str) -> Result<PathBuf> {
        let rel = Path::new(uri);
        if rel.is_absolute() {
            return Err(QError::PathOutsideRoot {
                requested: uri.to_string(),
            });
        }
        for c in rel.components() {
            match c {
                Component::Normal(_) | Component::CurDir => {}
                _ => {
                    return Err(QError::PathOutsideRoot {
                        requested: uri.to_string(),
                    })
                }
            }
        }
        let joined = self.root.join(rel);
        let canonical = joined.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => QError::NotFound(format!("{uri} (under {:?})", self.root)),
            _ => QError::Io {
                path: joined.clone(),
                source: e,
            },
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(QError::PathOutsideRoot {
                requested: uri.to_string(),
            });
        }
        Ok(canonical)
    }

    fn map_for(&self, uri: &str) -> Result<Arc<Mmap>> {
        if let Some(m) = self.maps.lock().unwrap().get(uri) {
            return Ok(Arc::clone(m));
        }
        let path = self.resolve(uri)?;
        let file = File::open(&path).map_err(|e| QError::Io {
            path: path.clone(),
            source: e,
        })?;
        // SAFETY: the artifact plane is immutable by contract (ARCHITECTURE.md
        // §2.1: "Never rewritten in place"). Concurrent external truncation
        // would be a violation of that contract, not a supported scenario.
        let map = unsafe { Mmap::map(&file) }.map_err(|e| QError::Io { path, source: e })?;
        let map = Arc::new(map);
        self.maps
            .lock()
            .unwrap()
            .insert(uri.to_string(), Arc::clone(&map));
        Ok(map)
    }
}

impl ModelSource for LocalFsSource {
    fn manifest(&self) -> Result<ModelManifest> {
        let mut files = Vec::new();
        let dir = std::fs::read_dir(&self.root).map_err(|e| QError::Io {
            path: self.root.clone(),
            source: e,
        })?;
        for entry in dir {
            let entry = entry.map_err(|e| QError::Io {
                path: self.root.clone(),
                source: e,
            })?;
            let meta = entry.metadata().map_err(|e| QError::Io {
                path: entry.path(),
                source: e,
            })?;
            if !meta.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            files.push(SourceFile {
                kind: ArtifactKind::classify(&name),
                uri: name,
                length: meta.len(),
            });
        }
        files.sort_by(|a, b| a.uri.cmp(&b.uri));

        // config.json is small by construction; read it eagerly so resolvers do
        // not each have to.
        let config = if files.iter().any(|f| f.kind == ArtifactKind::Config) {
            let path = self.root.join("config.json");
            let text = std::fs::read_to_string(&path).map_err(|e| QError::Io {
                path: path.clone(),
                source: e,
            })?;
            Some(serde_json::from_str(&text).map_err(|e| QError::json("config.json", e))?)
        } else {
            None
        };

        Ok(ModelManifest {
            source_key: self.source_key.clone(),
            root_uri: self.root.to_string_lossy().to_string(),
            revision: self.revision.clone(),
            files,
            config,
        })
    }

    fn read_range(&self, uri: &str, offset: u64, length: u64) -> Result<ByteStream> {
        let map = self.map_for(uri)?;
        let file_len = map.len() as u64;
        let end = offset.checked_add(length).ok_or_else(|| QError::RangeOutOfBounds {
            uri: uri.to_string(),
            start: offset,
            end: u64::MAX,
            length: file_len,
        })?;
        if end > file_len {
            return Err(QError::RangeOutOfBounds {
                uri: uri.to_string(),
                start: offset,
                end,
                length: file_len,
            });
        }
        Ok(ByteStream::new(
            length,
            Box::new(MmapWindow {
                map,
                pos: offset as usize,
                end: end as usize,
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::MemoryBudget;
    use crate::manifest::ModelSourceExt;

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/tiny-llama-2shard")
            .canonicalize()
            .expect("fixtures/tiny-llama-2shard must exist; run fixtures/generate_fixtures.py")
    }

    #[test]
    fn manifest_lists_artifacts_without_reading_payload() {
        let src = LocalFsSource::open(fixture_dir()).unwrap();
        let m = src.manifest().unwrap();
        assert_eq!(m.shards().count(), 2);
        assert!(m.shard_index().is_some());
        assert_eq!(m.model_type().as_deref(), Some("llama"));
        assert_eq!(m.config_u64("num_hidden_layers"), Some(12));
        // Lengths come from directory metadata, so each shard is ~600 KB
        // without any payload having been read.
        assert!(m.shards().all(|s| s.length > 100_000));
    }

    #[test]
    fn model_id_is_stable_across_reopen() {
        let a = LocalFsSource::open(fixture_dir()).unwrap().manifest().unwrap();
        let b = LocalFsSource::open(fixture_dir()).unwrap().manifest().unwrap();
        assert_eq!(a.model_id(), b.model_id());
    }

    #[test]
    fn range_read_returns_exactly_the_window() {
        let src = LocalFsSource::open(fixture_dir()).unwrap();
        let bytes = src
            .read_range_buffered(
                "model-00001-of-00002.safetensors",
                0,
                8,
                &MemoryBudget::single_read(),
            )
            .unwrap();
        assert_eq!(bytes.len(), 8);
        // First 8 bytes are the LE u64 header length.
        let hlen = u64::from_le_bytes(bytes.try_into().unwrap());
        assert!(hlen > 0 && hlen < 1_000_000);
    }

    #[test]
    fn range_past_end_of_file_is_rejected() {
        let src = LocalFsSource::open(fixture_dir()).unwrap();
        let err = src
            .read_range("model-00001-of-00002.safetensors", u64::MAX - 4, 8)
            .unwrap_err();
        assert!(matches!(err, QError::RangeOutOfBounds { .. }));
    }

    #[test]
    fn path_traversal_is_refused() {
        let src = LocalFsSource::open(fixture_dir()).unwrap();
        assert!(matches!(
            src.resolve("../generate_fixtures.py"),
            Err(QError::PathOutsideRoot { .. })
        ));
        assert!(matches!(
            src.resolve("/etc/passwd"),
            Err(QError::PathOutsideRoot { .. })
        ));
        assert!(src.resolve("config.json").is_ok());
    }

    #[test]
    fn missing_file_is_not_found_not_io_panic() {
        let src = LocalFsSource::open(fixture_dir()).unwrap();
        assert!(matches!(
            src.read_range("model-00099-of-00002.safetensors", 0, 1),
            Err(QError::NotFound(_))
        ));
    }
}
