//! Data plane: **Artifact Plane** (ARCHITECTURE.md §2.1, §4.1).
//!
//! HTTP Range-based [`ModelSource`] for remote checkpoints (Hugging Face).
//!
//! The *range arithmetic and header construction* are implemented and tested
//! here; the network transport is left as a pluggable [`RangeFetcher`] and
//! ships with no networking implementation in this pass. That is deliberate on
//! two counts: `docs/TESTING.md` bans network in default unit tests, and a
//! fabricated transport would be exactly the kind of plausible-looking stub
//! §20 forbids. [`NoNetworkFetcher`] therefore returns
//! [`QError::NotImplemented`] with requirement `SRC-008`.
//!
//! To wire a real transport, implement [`RangeFetcher`] over any HTTP client
//! and pass it to [`HttpRangeSource::new`]; nothing else changes.

use crate::error::{QError, Result};
use crate::manifest::{ByteStream, ModelManifest, ModelSource};

/// An inclusive HTTP byte range, as it appears in the `Range` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpByteRange {
    pub first: u64,
    /// Inclusive last byte — HTTP ranges are inclusive, SafeTensors offsets are
    /// half-open. Getting this wrong is the classic off-by-one, so the
    /// conversion lives in one tested place.
    pub last: u64,
}

impl HttpByteRange {
    /// Build from a half-open `offset..offset+length` window.
    pub fn from_offset_length(offset: u64, length: u64) -> Result<Self> {
        if length == 0 {
            return Err(QError::malformed(
                "http range",
                "zero-length ranges are not expressible as an HTTP Range header",
            ));
        }
        let last = offset
            .checked_add(length - 1)
            .ok_or_else(|| QError::malformed("http range", "offset + length overflows u64"))?;
        Ok(Self { first: offset, last })
    }

    pub fn header_value(&self) -> String {
        format!("bytes={}-{}", self.first, self.last)
    }

    pub fn length(&self) -> u64 {
        self.last - self.first + 1
    }
}

/// Transport for a single ranged GET.
pub trait RangeFetcher: Send + Sync {
    /// Fetch `range` of `url`. Implementations must return exactly
    /// `range.length()` bytes or an error — a short read is a failure, never a
    /// silently truncated success.
    fn fetch(&self, url: &str, range: HttpByteRange) -> Result<Vec<u8>>;

    /// Fetch the manifest describing the remote artifact set.
    fn manifest(&self, base_url: &str) -> Result<ModelManifest>;
}

/// The default transport: refuses, loudly.
pub struct NoNetworkFetcher;

impl RangeFetcher for NoNetworkFetcher {
    fn fetch(&self, url: &str, range: HttpByteRange) -> Result<Vec<u8>> {
        Err(QError::not_implemented(
            "SRC-008",
            format!(
                "HTTP Range transport is not built in this pass (wanted {} from {url}). \
                 Implement q_source::http::RangeFetcher and pass it to HttpRangeSource::new. \
                 See ARCHITECTURE.md §4.1.",
                range.header_value()
            ),
        ))
    }

    fn manifest(&self, base_url: &str) -> Result<ModelManifest> {
        Err(QError::not_implemented(
            "SRC-008",
            format!(
                "remote manifest discovery is not built in this pass (base {base_url}). \
                 See ARCHITECTURE.md §4.1."
            ),
        ))
    }
}

/// A [`ModelSource`] backed by HTTP Range requests.
pub struct HttpRangeSource {
    base_url: String,
    fetcher: Box<dyn RangeFetcher>,
}

impl HttpRangeSource {
    pub fn new(base_url: impl Into<String>, fetcher: Box<dyn RangeFetcher>) -> Self {
        Self {
            base_url: base_url.into(),
            fetcher,
        }
    }

    /// A source that will refuse every read. Useful for asserting that a code
    /// path does *not* touch the network.
    pub fn offline(base_url: impl Into<String>) -> Self {
        Self::new(base_url, Box::new(NoNetworkFetcher))
    }

    fn url_for(&self, uri: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), uri)
    }
}

impl ModelSource for HttpRangeSource {
    fn manifest(&self) -> Result<ModelManifest> {
        self.fetcher.manifest(&self.base_url)
    }

    fn read_range(&self, uri: &str, offset: u64, length: u64) -> Result<ByteStream> {
        let range = HttpByteRange::from_offset_length(offset, length)?;
        let bytes = self.fetcher.fetch(&self.url_for(uri), range)?;
        if bytes.len() as u64 != length {
            return Err(QError::malformed(
                uri,
                format!("range fetch returned {} bytes, expected {length}", bytes.len()),
            ));
        }
        Ok(ByteStream::from_vec(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// In-memory transport: exercises the range arithmetic with no network.
    struct InMemoryFetcher {
        files: HashMap<String, Vec<u8>>,
        seen: std::sync::Mutex<Vec<String>>,
    }

    impl RangeFetcher for InMemoryFetcher {
        fn fetch(&self, url: &str, range: HttpByteRange) -> Result<Vec<u8>> {
            self.seen.lock().unwrap().push(range.header_value());
            let data = self
                .files
                .get(url)
                .ok_or_else(|| QError::NotFound(url.to_string()))?;
            let last = range.last as usize;
            if last >= data.len() {
                return Err(QError::RangeOutOfBounds {
                    uri: url.to_string(),
                    start: range.first,
                    end: range.last + 1,
                    length: data.len() as u64,
                });
            }
            Ok(data[range.first as usize..=last].to_vec())
        }

        fn manifest(&self, _base_url: &str) -> Result<ModelManifest> {
            Err(QError::not_implemented("SRC-008", "test fetcher"))
        }
    }

    #[test]
    fn half_open_offsets_become_inclusive_http_ranges() {
        let r = HttpByteRange::from_offset_length(100, 4).unwrap();
        assert_eq!(r.header_value(), "bytes=100-103");
        assert_eq!(r.length(), 4);
    }

    #[test]
    fn zero_length_range_is_rejected() {
        assert!(HttpByteRange::from_offset_length(0, 0).is_err());
    }

    #[test]
    fn ranged_read_returns_exactly_the_window() {
        let mut files = HashMap::new();
        files.insert(
            "https://example.invalid/m/model.safetensors".to_string(),
            (0u8..=255).collect::<Vec<u8>>(),
        );
        let fetcher = InMemoryFetcher {
            files,
            seen: Default::default(),
        };
        let src = HttpRangeSource::new("https://example.invalid/m", Box::new(fetcher));
        let mut out = Vec::new();
        src.read_range("model.safetensors", 10, 4)
            .unwrap()
            .copy_to(&mut out)
            .unwrap();
        assert_eq!(out, vec![10, 11, 12, 13]);
    }

    #[test]
    fn offline_source_refuses_with_a_requirement_id() {
        let src = HttpRangeSource::offline("https://example.invalid/m");
        let err = src.read_range("model.safetensors", 0, 4).unwrap_err();
        assert_eq!(err.requirement_id(), Some("SRC-008"));
        assert!(err.to_string().contains("bytes=0-3"));
    }
}
