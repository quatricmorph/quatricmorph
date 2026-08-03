//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §6.2).
//!
//! The contextual-alias grammar.
//!
//! ARCHITECTURE.md §6.2 requires these forms to parse:
//!
//! ```text
//! Q[10][100, 42]
//! K[10][0:256, 0:256]
//! MLP.down[24][:]
//! Expert[12, 37].up[0:128, :]
//! Att[10][100]
//! ```
//!
//! ## The one disambiguation rule
//!
//! **The first bracket group in an alias is structural; every later group is
//! the element selector.**
//!
//! That single rule explains all five forms above, regardless of which segment
//! the brackets are attached to:
//!
//! | input                      | structural | element selector |
//! |----------------------------|------------|------------------|
//! | `Q[10][100,42]`            | `[10]`     | `[100,42]`       |
//! | `MLP.down[24][:]`          | `[24]`     | `[:]`            |
//! | `Expert[12,37].up[0:128,:]`| `[12,37]`  | `[0:128,:]`      |
//! | `Q[10]`                    | `[10]`     | none             |
//!
//! A structural group of one is a layer index; a group of two is
//! `(layer, expert)`.
//!
//! Parsing is separate from resolution: this module turns text into a
//! [`ParsedAlias`], and [`crate::resolver`] turns that into tensor candidates.
//! An alias that names several tensors yields *candidates*, never a silent pick.

use crate::address::{Cursor, ElementSelector, IndexTerm};
use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A parsed contextual alias, before any model is consulted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedAlias {
    /// The original text, echoed back in resolution results.
    pub input: String,
    /// Dotted alias name with subscripts stripped, e.g. `Q`, `MLP.down`,
    /// `Expert.up`.
    pub alias: String,
    pub layer_index: Option<u32>,
    pub expert_index: Option<u32>,
    pub selector: Option<ElementSelector>,
}

impl ParsedAlias {
    /// Parse an alias. Rejects rather than guesses.
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(QError::QueryRejected("empty alias".into()));
        }
        let mut c = Cursor::new(trimmed);
        let mut names: Vec<String> = Vec::new();
        let mut groups: Vec<Vec<IndexTerm>> = Vec::new();

        loop {
            names.push(c.ident_public()?);
            while c.peek_open_bracket() {
                groups.push(c.subscript()?);
            }
            if !c.eat_dot() {
                break;
            }
        }
        c.expect_end()?;

        let alias = names.join(".");
        let (mut layer_index, mut expert_index) = (None, None);
        let mut selector = None;

        if let Some(structural) = groups.first() {
            let points: Vec<u64> = structural
                .iter()
                .map(|t| match t {
                    IndexTerm::Point(i) => Ok(*i),
                    _ => Err(QError::QueryRejected(format!(
                        "structural subscript in `{trimmed}` must be integers, not a range"
                    ))),
                })
                .collect::<Result<_>>()?;
            match points.len() {
                1 => layer_index = Some(points[0] as u32),
                2 => {
                    layer_index = Some(points[0] as u32);
                    expert_index = Some(points[1] as u32);
                }
                n => {
                    return Err(QError::QueryRejected(format!(
                        "structural subscript in `{trimmed}` has {n} entries; expected 1 (layer) or 2 (layer, expert)"
                    )))
                }
            }
        }
        if groups.len() > 2 {
            return Err(QError::QueryRejected(format!(
                "`{trimmed}` has {} subscript groups; at most 2 are meaningful (structural, element)",
                groups.len()
            )));
        }
        if let Some(element) = groups.get(1) {
            selector = Some(ElementSelector(element.clone()));
        }

        Ok(ParsedAlias {
            input: trimmed.to_string(),
            alias,
            layer_index,
            expert_index,
            selector,
        })
    }

    /// True when the alias names a whole tensor with no region selected.
    pub fn is_whole_tensor(&self) -> bool {
        self.selector.is_none()
    }
}

impl fmt::Display for ParsedAlias {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_five_architecture_md_forms() {
        let q = ParsedAlias::parse("Q[10][100, 42]").unwrap();
        assert_eq!(q.alias, "Q");
        assert_eq!(q.layer_index, Some(10));
        assert_eq!(
            q.selector,
            Some(ElementSelector(vec![
                IndexTerm::Point(100),
                IndexTerm::Point(42)
            ]))
        );

        let k = ParsedAlias::parse("K[10][0:256, 0:256]").unwrap();
        assert_eq!(k.alias, "K");
        assert_eq!(k.layer_index, Some(10));
        assert_eq!(k.selector.unwrap().rank(), 2);

        let mlp = ParsedAlias::parse("MLP.down[24][:]").unwrap();
        assert_eq!(mlp.alias, "MLP.down");
        assert_eq!(mlp.layer_index, Some(24));
        assert_eq!(mlp.selector, Some(ElementSelector(vec![IndexTerm::All])));

        let expert = ParsedAlias::parse("Expert[12, 37].up[0:128, :]").unwrap();
        assert_eq!(expert.alias, "Expert.up");
        assert_eq!(expert.layer_index, Some(12));
        assert_eq!(expert.expert_index, Some(37));
        assert_eq!(
            expert.selector,
            Some(ElementSelector(vec![
                IndexTerm::Range {
                    start: Some(0),
                    end: Some(128)
                },
                IndexTerm::All
            ]))
        );

        let att = ParsedAlias::parse("Att[10][100]").unwrap();
        assert_eq!(att.alias, "Att");
        assert_eq!(att.layer_index, Some(10));
        assert_eq!(
            att.selector,
            Some(ElementSelector(vec![IndexTerm::Point(100)]))
        );
    }

    #[test]
    fn alias_without_element_selector_names_a_whole_tensor() {
        let a = ParsedAlias::parse("Q[10]").unwrap();
        assert!(a.is_whole_tensor());
        assert_eq!(a.layer_index, Some(10));
        assert_eq!(a.selector, None);
    }

    #[test]
    fn alias_without_any_subscript_parses() {
        let a = ParsedAlias::parse("Embed").unwrap();
        assert_eq!(a.alias, "Embed");
        assert_eq!(a.layer_index, None);
        assert!(a.is_whole_tensor());
    }

    #[test]
    fn structural_subscript_rejects_ranges() {
        assert!(matches!(
            ParsedAlias::parse("Q[0:10][100]"),
            Err(QError::QueryRejected(_))
        ));
    }

    #[test]
    fn more_than_two_structural_entries_is_rejected() {
        assert!(ParsedAlias::parse("Expert[1,2,3].up").is_err());
    }

    #[test]
    fn more_than_two_subscript_groups_is_rejected() {
        assert!(ParsedAlias::parse("Q[10][1][2]").is_err());
    }

    #[test]
    fn invalid_syntax_is_rejected() {
        for bad in [
            "",
            "  ",
            "Q[10",
            "Q[]",
            "Q[10][",
            "[10]",
            "Q..down",
            "Q[10] junk",
        ] {
            assert!(
                ParsedAlias::parse(bad).is_err(),
                "expected `{bad}` to be rejected"
            );
        }
    }

    #[test]
    fn input_is_echoed_verbatim_for_reporting() {
        let a = ParsedAlias::parse("  Q[10][100,42]  ").unwrap();
        assert_eq!(a.input, "Q[10][100,42]");
        assert_eq!(a.to_string(), "Q[10][100,42]");
    }
}
