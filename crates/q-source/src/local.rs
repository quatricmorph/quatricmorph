//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §4.1).
//!
//! Memory-mapped local-disk [`ModelSource`].
//!
//! Reads go through `mmap`, so a range read costs the pages it touches and
//! nothing else — opening a 600 GB shard and reading four bytes from the middle
//! of it does not allocate 600 GB. Maps are cached per file behind an `Arc` so
//! repeated reads of the same shard do not re-map it.
//!
//! # The one case where mapping is the wrong default
//!
//! Mapped pages **count toward RSS while they are resident**
//! (`.plan/MEMORY_BUDGET.md` §3). For a scalar or slice read that is exactly
//! what is wanted: a handful of pages, evictable, effectively free. For a pass
//! that touches *every* byte of a checkpoint it is the opposite — the kernel has
//! no reason to evict anything on a machine under no pressure, so the process's
//! peak RSS approaches the file size even though the pass never holds more than
//! one block. A residency measurement taken that way measures the page cache,
//! not the streamer.
//!
//! [`ReadMode::Pread`] is the answer: `seek` + `read` into the caller's buffer,
//! no mapping, so nothing but the streamer's own buffers can appear in the
//! measurement. It is **opt-in** — [`LocalFsSource::open`] still maps, and every
//! existing caller keeps the behaviour it was written against. `QM-0101` selects
//! it for `q stream`.
//!
//! Security: [`LocalFsSource`] confines every access to its configured root
//! (`SEC-001`) in **both** modes. A `uri` that escapes the root via `..`, an
//! absolute path, or a symlink is rejected with [`QError::PathOutsideRoot`].

use crate::error::{QError, Result};
use crate::manifest::{ArtifactKind, ByteStream, ModelManifest, ModelSource, SourceFile};
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

/// `offset + length`, refused if it overruns `file_len` or overflows.
///
/// Shared by both read modes so a range refusal is identical whichever one is in
/// use — a bound that held only on the mapped path would be a security property
/// that depended on a performance flag.
fn bounded_end(uri: &str, offset: u64, length: u64, file_len: u64) -> Result<u64> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| QError::RangeOutOfBounds {
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
    Ok(end)
}

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

/// A read-only window over an open file, read by `seek` + `read`.
///
/// Allocates nothing: it fills the caller's buffer directly, so a window over a
/// 16 GiB range costs the same as a window over 1 KiB. The `Mutex` is what makes
/// one shared descriptor safe to seek from several windows, and is also what
/// keeps [`ByteStream`]'s `Send` bound satisfiable.
struct FileWindow {
    file: Arc<Mutex<File>>,
    pos: u64,
    end: u64,
}

impl Read for FileWindow {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining = self.end.saturating_sub(self.pos);
        if remaining == 0 || buf.is_empty() {
            return Ok(0);
        }
        let want = remaining.min(buf.len() as u64) as usize;
        let mut file = self
            .file
            .lock()
            .map_err(|_| std::io::Error::other("the shard descriptor mutex was poisoned"))?;
        file.seek(SeekFrom::Start(self.pos))?;
        let read = file.read(&mut buf[..want])?;
        self.pos += read as u64;
        Ok(read)
    }
}

/// One cached shard descriptor and the file length range checks are made against.
///
/// The length is cached alongside the handle so a per-row range read does not
/// `stat` the shard 256 times per block.
type SharedShard = (Arc<Mutex<File>>, u64);

/// How a [`LocalFsSource`] reads payload bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    /// Memory-mapped. The default, and right for point and slice reads.
    Mmap,
    /// `seek` + `read` into the caller's buffer. No mapping, so no mapped page
    /// enters a residency measurement.
    Pread,
}

/// A checkpoint directory on local disk.
pub struct LocalFsSource {
    root: PathBuf,
    source_key: String,
    revision: String,
    mode: ReadMode,
    maps: Mutex<HashMap<String, Arc<Mmap>>>,
    /// Open descriptors and their lengths, for [`ReadMode::Pread`]. Cached for
    /// the same reason maps are: a per-row range read must not re-open the shard
    /// 256 times per block.
    files: Mutex<HashMap<String, SharedShard>>,
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
            mode: ReadMode::Mmap,
            maps: Mutex::new(HashMap::new()),
            files: Mutex::new(HashMap::new()),
        })
    }

    /// As [`LocalFsSource::open`], but reading by `seek` + `read` rather than by
    /// mapping.
    ///
    /// For a pass that touches every byte of a checkpoint. See the module
    /// documentation for why mapping is the wrong default *only* in that case.
    pub fn open_without_mapping(root: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::open(root)?.with_read_mode(ReadMode::Pread))
    }

    pub fn with_read_mode(mut self, mode: ReadMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn read_mode(&self) -> ReadMode {
        self.mode
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
            std::io::ErrorKind::NotFound => {
                QError::NotFound(format!("{uri} (under {:?})", self.root))
            }
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

    /// An open descriptor for `uri` and its length, cached.
    fn file_for(&self, uri: &str) -> Result<SharedShard> {
        if let Some(entry) = self.files.lock().unwrap().get(uri) {
            return Ok((Arc::clone(&entry.0), entry.1));
        }
        let path = self.resolve(uri)?;
        let file = File::open(&path).map_err(|e| QError::Io {
            path: path.clone(),
            source: e,
        })?;
        let length = file
            .metadata()
            .map_err(|e| QError::Io {
                path: path.clone(),
                source: e,
            })?
            .len();
        let handle = Arc::new(Mutex::new(file));
        self.files
            .lock()
            .unwrap()
            .insert(uri.to_string(), (Arc::clone(&handle), length));
        Ok((handle, length))
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
        // Both modes validate the range against the real file length before
        // handing back a reader, so an out-of-bounds range is refused the same
        // way whichever mode is in use.
        match self.mode {
            ReadMode::Mmap => {
                let map = self.map_for(uri)?;
                let end = bounded_end(uri, offset, length, map.len() as u64)?;
                Ok(ByteStream::new(
                    length,
                    Box::new(MmapWindow {
                        map,
                        pos: offset as usize,
                        end: end as usize,
                    }),
                ))
            }
            ReadMode::Pread => {
                let (file, file_len) = self.file_for(uri)?;
                let end = bounded_end(uri, offset, length, file_len)?;
                Ok(ByteStream::new(
                    length,
                    Box::new(FileWindow {
                        file,
                        pos: offset,
                        end,
                    }),
                ))
            }
        }
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
        let a = LocalFsSource::open(fixture_dir())
            .unwrap()
            .manifest()
            .unwrap();
        let b = LocalFsSource::open(fixture_dir())
            .unwrap()
            .manifest()
            .unwrap();
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

    // --- ReadMode::Pread (`QM-0101`) -----------------------------------------

    const SHARD: &str = "model-00001-of-00002.safetensors";

    #[test]
    fn the_default_read_mode_still_maps_and_pread_is_opt_in() {
        // Every pre-existing caller keeps mapping. A residency flag that silently
        // changed how `q value` reads would be a behaviour change dressed up as
        // an optimisation.
        assert_eq!(
            LocalFsSource::open(fixture_dir()).unwrap().read_mode(),
            ReadMode::Mmap
        );
        assert_eq!(
            LocalFsSource::open_without_mapping(fixture_dir())
                .unwrap()
                .read_mode(),
            ReadMode::Pread
        );
    }

    /// The property that makes the read mode a measurement choice rather than a
    /// correctness one: the bytes are identical.
    #[test]
    fn pread_and_mmap_return_byte_identical_windows_at_every_offset_tried() {
        let mapped = LocalFsSource::open(fixture_dir()).unwrap();
        let pread = LocalFsSource::open_without_mapping(fixture_dir()).unwrap();
        // Offset 0 (the header length prefix), an interior odd offset, and a
        // window that ends exactly at the last byte of the file.
        let file_len = std::fs::metadata(fixture_dir().join(SHARD)).unwrap().len();
        for (offset, length) in [(0u64, 8u64), (1, 3), (517, 1024), (file_len - 5, 5)] {
            let a = mapped
                .read_range_buffered(SHARD, offset, length, &MemoryBudget::single_read())
                .unwrap();
            let b = pread
                .read_range_buffered(SHARD, offset, length, &MemoryBudget::single_read())
                .unwrap();
            assert_eq!(a.len() as u64, length, "mapped window {offset}+{length}");
            assert_eq!(a, b, "window {offset}+{length} differs between read modes");
        }
    }

    #[test]
    fn a_pread_window_is_read_incrementally_without_materialising_the_range() {
        // The window fills the caller's buffer directly, so a caller reading 4
        // bytes at a time never causes the range to be allocated. Asserted by
        // behaviour: two successive reads advance rather than restart.
        let pread = LocalFsSource::open_without_mapping(fixture_dir()).unwrap();
        let mut stream = pread.read_range(SHARD, 0, 8).unwrap();
        let mut first = [0u8; 4];
        let mut second = [0u8; 4];
        stream.read_exact(&mut first).unwrap();
        stream.read_exact(&mut second).unwrap();
        let whole = pread
            .read_range_buffered(SHARD, 0, 8, &MemoryBudget::single_read())
            .unwrap();
        assert_eq!(&whole[..4], &first);
        assert_eq!(&whole[4..], &second);
        // And it stops at the window's end rather than running on into the file.
        let mut past = [0u8; 4];
        assert_eq!(stream.read(&mut past).unwrap(), 0);
    }

    /// `SEC-001` is a security property and must not depend on a performance
    /// flag. Both refusals are re-checked in the non-mapping mode.
    #[test]
    fn pread_mode_enforces_the_same_root_confinement_and_range_bounds() {
        let pread = LocalFsSource::open_without_mapping(fixture_dir()).unwrap();
        assert!(matches!(
            pread.resolve("../generate_fixtures.py"),
            Err(QError::PathOutsideRoot { .. })
        ));
        assert!(matches!(
            pread.resolve("/etc/passwd"),
            Err(QError::PathOutsideRoot { .. })
        ));
        assert!(matches!(
            pread.read_range(SHARD, u64::MAX - 4, 8),
            Err(QError::RangeOutOfBounds { .. })
        ));
        let file_len = std::fs::metadata(fixture_dir().join(SHARD)).unwrap().len();
        assert!(matches!(
            pread.read_range(SHARD, file_len - 1, 2),
            Err(QError::RangeOutOfBounds { .. })
        ));
        assert!(matches!(
            pread.read_range("model-00099-of-00002.safetensors", 0, 1),
            Err(QError::NotFound(_))
        ));
    }

    #[test]
    fn pread_windows_over_one_shard_share_a_single_descriptor_and_do_not_interleave() {
        // 256 range reads per block is the streaming path's shape, so a
        // descriptor cache that leaked a handle per read would exhaust the
        // process's file limit on a real checkpoint. Two live windows over the
        // same shard must also not disturb each other's position.
        let pread = LocalFsSource::open_without_mapping(fixture_dir()).unwrap();
        let mut a = pread.read_range(SHARD, 0, 4).unwrap();
        let mut b = pread.read_range(SHARD, 4, 4).unwrap();
        let mut ab = [0u8; 4];
        let mut bb = [0u8; 4];
        // Interleaved reads: a, b, a, b.
        a.read_exact(&mut ab[..2]).unwrap();
        b.read_exact(&mut bb[..2]).unwrap();
        a.read_exact(&mut ab[2..]).unwrap();
        b.read_exact(&mut bb[2..]).unwrap();
        let whole = pread
            .read_range_buffered(SHARD, 0, 8, &MemoryBudget::single_read())
            .unwrap();
        assert_eq!(&whole[..4], &ab);
        assert_eq!(&whole[4..], &bb);
    }
}
