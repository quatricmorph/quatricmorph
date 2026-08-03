//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §6.1).
//!
//! Canonical addresses and element selectors.
//!
//! A canonical address is the unique, reusable name for a tensor or a region of
//! one (ARCHITECTURE.md §6.1):
//!
//! ```text
//! model.layers[10].self_attention.query_projection.weight[100,42]
//! ```
//!
//! Two subscript positions exist and mean different things:
//!
//! * **structural** subscripts attach to path segments (`layers[10]`,
//!   `experts[37]`) and select part of the model;
//! * the **element selector** is the trailing subscript after the parameter
//!   (`weight[100,42]`) and selects part of the tensor.
//!
//! The parser is hand-written recursive descent — see
//! `docs/decisions/ADR-005-hand-written-parsers.md` for why no parser generator
//! is used.

use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// One entry inside a subscript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexTerm {
    /// `42`
    Point(u64),
    /// `0:256`, `:256`, `0:`
    Range {
        start: Option<u64>,
        end: Option<u64>,
    },
    /// `:`
    All,
}

impl IndexTerm {
    pub fn is_point(self) -> bool {
        matches!(self, IndexTerm::Point(_))
    }

    /// Concrete `[start, end)` bounds against an axis of length `dim`.
    pub fn bounds(self, dim: u64) -> (u64, u64) {
        match self {
            IndexTerm::Point(i) => (i, i + 1),
            IndexTerm::All => (0, dim),
            IndexTerm::Range { start, end } => (start.unwrap_or(0), end.unwrap_or(dim)),
        }
    }
}

impl fmt::Display for IndexTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexTerm::Point(i) => write!(f, "{i}"),
            IndexTerm::All => write!(f, ":"),
            IndexTerm::Range { start, end } => {
                if let Some(s) = start {
                    write!(f, "{s}")?;
                }
                write!(f, ":")?;
                if let Some(e) = end {
                    write!(f, "{e}")?;
                }
                Ok(())
            }
        }
    }
}

/// The trailing `[...]` that selects a region of a tensor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElementSelector(pub Vec<IndexTerm>);

impl ElementSelector {
    pub fn rank(&self) -> usize {
        self.0.len()
    }

    /// True when every term is a point *and* the selector covers every axis —
    /// i.e. it names exactly one element.
    pub fn is_scalar_for(&self, shape: &[u64]) -> bool {
        self.0.len() == shape.len() && self.0.iter().all(|t| t.is_point())
    }

    /// The point index, if this selector names exactly one element.
    pub fn as_point_index(&self, shape: &[u64]) -> Option<Vec<u64>> {
        if !self.is_scalar_for(shape) {
            return None;
        }
        Some(
            self.0
                .iter()
                .map(|t| match t {
                    IndexTerm::Point(i) => *i,
                    _ => unreachable!("is_scalar_for checked every term"),
                })
                .collect(),
        )
    }

    /// Resolve to `(rows, columns)` half-open ranges against a rank-2 shape.
    ///
    /// A selector with fewer terms than the rank is padded with `All` on the
    /// right, so `[100]` on a `[128, 48]` tensor means "row 100, all columns" —
    /// exactly the resolution ARCHITECTURE.md §6.2 specifies for `Att[10][100]`.
    pub fn resolve_2d(&self, shape: &[u64]) -> Result<((u64, u64), (u64, u64))> {
        if shape.len() != 2 {
            return Err(QError::QueryRejected(format!(
                "2-D selector applied to a rank-{} tensor",
                shape.len()
            )));
        }
        if self.0.len() > 2 {
            return Err(QError::QueryRejected(format!(
                "selector has {} terms but the tensor has rank 2",
                self.0.len()
            )));
        }
        let row_term = self.0.first().copied().unwrap_or(IndexTerm::All);
        let col_term = self.0.get(1).copied().unwrap_or(IndexTerm::All);
        let rows = row_term.bounds(shape[0]);
        let cols = col_term.bounds(shape[1]);
        for ((s, e), dim, axis) in [(rows, shape[0], "row"), (cols, shape[1], "column")] {
            if e <= s {
                return Err(QError::QueryRejected(format!("empty {axis} range {s}:{e}")));
            }
            if e > dim {
                return Err(QError::IndexOutOfBounds {
                    tensor: "<selector>".into(),
                    index: vec![e - 1],
                    shape: shape.to_vec(),
                });
            }
        }
        Ok((rows, cols))
    }

    /// Number of elements this selector names against `shape`.
    pub fn element_count(&self, shape: &[u64]) -> u64 {
        let mut n = 1u64;
        for (i, dim) in shape.iter().enumerate() {
            let term = self.0.get(i).copied().unwrap_or(IndexTerm::All);
            let (s, e) = term.bounds(*dim);
            n = n.saturating_mul(e.saturating_sub(s));
        }
        n
    }
}

impl fmt::Display for ElementSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, t) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{t}")?;
        }
        write!(f, "]")
    }
}

/// One `.`-separated segment of a canonical path, with optional structural
/// subscript, e.g. `layers[10]` or `self_attention`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathSegment {
    pub name: String,
    pub subscript: Option<Vec<u64>>,
}

impl fmt::Display for PathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if let Some(sub) = &self.subscript {
            write!(f, "[")?;
            for (i, v) in sub.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{v}")?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

/// A parsed canonical address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalAddress {
    pub segments: Vec<PathSegment>,
    pub selector: Option<ElementSelector>,
}

impl CanonicalAddress {
    /// The address with its element selector removed — the tensor's own name.
    pub fn tensor_path(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Value of the first `layers[N]` subscript, if present.
    pub fn layer_index(&self) -> Option<u32> {
        self.segments
            .iter()
            .find(|s| s.name == "layers")
            .and_then(|s| s.subscript.as_ref())
            .and_then(|v| v.first())
            .map(|v| *v as u32)
    }

    pub fn expert_index(&self) -> Option<u32> {
        self.segments
            .iter()
            .find(|s| s.name == "experts")
            .and_then(|s| s.subscript.as_ref())
            .and_then(|v| v.first())
            .map(|v| *v as u32)
    }

    /// Parse a canonical address.
    ///
    /// The trailing subscript is the element selector only when it contains a
    /// range or when it follows the final segment; structural subscripts must
    /// be plain integers.
    pub fn parse(input: &str) -> Result<Self> {
        let mut p = Cursor::new(input);
        let addr = p.parse_address()?;
        p.expect_end()?;
        Ok(addr)
    }
}

impl fmt::Display for CanonicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.tensor_path())?;
        if let Some(sel) = &self.selector {
            write!(f, "{sel}")?;
        }
        Ok(())
    }
}

/// Shared character cursor for the canonical-address and alias grammars.
pub(crate) struct Cursor<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn reject(&self, msg: impl fmt::Display) -> QError {
        QError::QueryRejected(format!("{msg} in `{}` at byte {}", self.src, self.pos))
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub(crate) fn expect_end(&mut self) -> Result<()> {
        self.skip_ws();
        if self.pos != self.bytes.len() {
            return Err(self.reject("unexpected trailing input"));
        }
        Ok(())
    }

    fn ident(&mut self) -> Result<String> {
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(self.reject("expected an identifier"));
        }
        Ok(self.src[start..self.pos].to_string())
    }

    fn integer(&mut self) -> Result<u64> {
        self.skip_ws();
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.pos += 1;
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(self.reject("expected an integer"));
        }
        self.src[start..self.pos]
            .parse()
            .map_err(|_| self.reject("integer out of range"))
    }

    /// Identifier, exposed for the alias grammar in [`crate::alias`].
    pub(crate) fn ident_public(&mut self) -> Result<String> {
        self.ident()
    }

    /// Whether the next non-whitespace character opens a subscript.
    pub(crate) fn peek_open_bracket(&mut self) -> bool {
        self.skip_ws();
        self.peek() == Some(b'[')
    }

    /// Consume a `.` separator if present.
    pub(crate) fn eat_dot(&mut self) -> bool {
        self.eat(b'.')
    }

    /// Parse `[ term (, term)* ]`, where a term may be a point or a range.
    pub(crate) fn subscript(&mut self) -> Result<Vec<IndexTerm>> {
        if !self.eat(b'[') {
            return Err(self.reject("expected `[`"));
        }
        let mut terms = Vec::new();
        loop {
            self.skip_ws();
            let term = if self.peek() == Some(b':') {
                self.pos += 1;
                self.skip_ws();
                if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    IndexTerm::Range {
                        start: None,
                        end: Some(self.integer()?),
                    }
                } else {
                    IndexTerm::All
                }
            } else {
                let n = self.integer()?;
                self.skip_ws();
                if self.peek() == Some(b':') {
                    self.pos += 1;
                    self.skip_ws();
                    if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                        IndexTerm::Range {
                            start: Some(n),
                            end: Some(self.integer()?),
                        }
                    } else {
                        IndexTerm::Range {
                            start: Some(n),
                            end: None,
                        }
                    }
                } else {
                    IndexTerm::Point(n)
                }
            };
            terms.push(term);
            if self.eat(b',') {
                continue;
            }
            if self.eat(b']') {
                break;
            }
            return Err(self.reject("expected `,` or `]`"));
        }
        if terms.is_empty() {
            return Err(self.reject("empty subscript"));
        }
        Ok(terms)
    }

    fn parse_address(&mut self) -> Result<CanonicalAddress> {
        let mut segments: Vec<PathSegment> = Vec::new();
        let mut selector: Option<ElementSelector> = None;

        loop {
            let name = self.ident()?;
            let mut subscript = None;
            self.skip_ws();
            if self.peek() == Some(b'[') {
                let terms = self.subscript()?;
                let all_points = terms.iter().all(|t| t.is_point());
                self.skip_ws();
                let is_last = self.peek() != Some(b'.');
                if is_last && (!all_points || !segments.is_empty() || terms.len() > 1) {
                    // Trailing subscript on the final segment is the element
                    // selector. `foo[3]` with no preceding segments is treated
                    // as structural so that bare `layers[3]` still parses.
                    selector = Some(ElementSelector(terms));
                } else if all_points {
                    subscript = Some(
                        terms
                            .iter()
                            .map(|t| match t {
                                IndexTerm::Point(i) => Ok(*i),
                                _ => Err(self.reject("structural subscripts must be integers")),
                            })
                            .collect::<Result<Vec<u64>>>()?,
                    );
                } else {
                    return Err(self.reject("structural subscripts must be integers"));
                }
            }
            segments.push(PathSegment { name, subscript });
            if selector.is_some() {
                break;
            }
            if !self.eat(b'.') {
                break;
            }
        }
        Ok(CanonicalAddress { segments, selector })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_architecture_md_example() {
        let a = CanonicalAddress::parse(
            "model.layers[10].self_attention.query_projection.weight[100,42]",
        )
        .unwrap();
        assert_eq!(
            a.tensor_path(),
            "model.layers[10].self_attention.query_projection.weight"
        );
        assert_eq!(a.layer_index(), Some(10));
        assert_eq!(
            a.selector,
            Some(ElementSelector(vec![
                IndexTerm::Point(100),
                IndexTerm::Point(42)
            ]))
        );
    }

    #[test]
    fn round_trips_through_display() {
        for s in [
            "model.layers[10].self_attention.query_projection.weight",
            "model.layers[10].self_attention.query_projection.weight[100,42]",
            "model.layers[3].moe.experts[37].down_projection.weight[0:128,:]",
            "model.embedding.token_embedding.weight",
        ] {
            let a = CanonicalAddress::parse(s).unwrap();
            assert_eq!(a.to_string(), s, "round trip failed for {s}");
        }
    }

    #[test]
    fn extracts_layer_and_expert_indices() {
        let a = CanonicalAddress::parse("model.layers[3].moe.experts[37].down_projection.weight")
            .unwrap();
        assert_eq!(a.layer_index(), Some(3));
        assert_eq!(a.expert_index(), Some(37));
    }

    #[test]
    fn parses_every_range_spelling() {
        let cases = [
            (
                "t.w[0:256,0:256]",
                vec![
                    IndexTerm::Range {
                        start: Some(0),
                        end: Some(256),
                    },
                    IndexTerm::Range {
                        start: Some(0),
                        end: Some(256),
                    },
                ],
            ),
            ("t.w[:]", vec![IndexTerm::All]),
            (
                "t.w[:128]",
                vec![IndexTerm::Range {
                    start: None,
                    end: Some(128),
                }],
            ),
            (
                "t.w[128:]",
                vec![IndexTerm::Range {
                    start: Some(128),
                    end: None,
                }],
            ),
            (
                "t.w[0:128,:]",
                vec![
                    IndexTerm::Range {
                        start: Some(0),
                        end: Some(128),
                    },
                    IndexTerm::All,
                ],
            ),
            ("t.w[100]", vec![IndexTerm::Point(100)]),
        ];
        for (src, want) in cases {
            let a = CanonicalAddress::parse(src).unwrap();
            assert_eq!(a.selector, Some(ElementSelector(want)), "for {src}");
        }
    }

    #[test]
    fn whitespace_inside_subscripts_is_tolerated() {
        let a = CanonicalAddress::parse("t.w[100, 42]").unwrap();
        assert_eq!(
            a.selector,
            Some(ElementSelector(vec![
                IndexTerm::Point(100),
                IndexTerm::Point(42)
            ]))
        );
    }

    #[test]
    fn scalar_detection_requires_full_rank_points() {
        let sel = ElementSelector(vec![IndexTerm::Point(100), IndexTerm::Point(42)]);
        assert!(sel.is_scalar_for(&[128, 48]));
        assert_eq!(sel.as_point_index(&[128, 48]).unwrap(), vec![100, 42]);

        // A single point on a rank-2 tensor is a row, not a scalar.
        let row = ElementSelector(vec![IndexTerm::Point(100)]);
        assert!(!row.is_scalar_for(&[128, 48]));
        assert!(row.as_point_index(&[128, 48]).is_none());
    }

    #[test]
    fn single_point_on_rank2_means_whole_row() {
        // ARCHITECTURE.md §6.2: Att[10][100] -> {row: 100, columns: "all"}
        let sel = ElementSelector(vec![IndexTerm::Point(100)]);
        assert_eq!(sel.resolve_2d(&[128, 48]).unwrap(), ((100, 101), (0, 48)));
        assert_eq!(sel.element_count(&[128, 48]), 48);
    }

    #[test]
    fn resolve_2d_handles_ranges_and_all() {
        let sel = ElementSelector(vec![
            IndexTerm::Range {
                start: Some(0),
                end: Some(128),
            },
            IndexTerm::All,
        ]);
        assert_eq!(sel.resolve_2d(&[256, 48]).unwrap(), ((0, 128), (0, 48)));
    }

    #[test]
    fn out_of_range_selector_is_rejected() {
        let sel = ElementSelector(vec![IndexTerm::Range {
            start: Some(0),
            end: Some(999),
        }]);
        assert!(matches!(
            sel.resolve_2d(&[128, 48]),
            Err(QError::IndexOutOfBounds { .. })
        ));
    }

    #[test]
    fn inverted_range_is_rejected() {
        let sel = ElementSelector(vec![IndexTerm::Range {
            start: Some(10),
            end: Some(2),
        }]);
        assert!(matches!(
            sel.resolve_2d(&[128, 48]),
            Err(QError::QueryRejected(_))
        ));
    }

    #[test]
    fn invalid_syntax_is_rejected_not_guessed() {
        for bad in [
            "model.layers[10",
            "model..weight",
            "model.layers[].weight",
            "model.layers[abc].weight",
            "model.weight[1,]",
            "",
            "model.weight[1] junk",
        ] {
            assert!(
                CanonicalAddress::parse(bad).is_err(),
                "expected `{bad}` to be rejected"
            );
        }
    }

    #[test]
    fn element_count_counts_the_selected_region() {
        let sel = ElementSelector(vec![
            IndexTerm::Range {
                start: Some(0),
                end: Some(4),
            },
            IndexTerm::Range {
                start: Some(0),
                end: Some(4),
            },
        ]);
        assert_eq!(sel.element_count(&[128, 48]), 16);
    }
}
