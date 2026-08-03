//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §7).
//!
//! WeightQL tokenizer.
//!
//! Hand-written, single pass, no dependencies. The token set is closed: there
//! is no token for a shell escape, a code block, or a raw-SQL passthrough,
//! because the language has no such construct (see
//! `docs/decisions/ADR-006-weightql-no-arbitrary-execution.md`).

use q_source::error::{QError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    /// A double-quoted string literal. Escapes: `\"` and `\\` only.
    Str(String),
    Int(u64),
    At,        // @
    Plus,      // +
    Minus,     // -
    Equals,    // =
    Comma,     // ,
    Colon,     // :
    Semicolon, // ;
    LParen,
    RParen,
    LBracket,
    RBracket,
    Eof,
}

impl Token {
    pub fn describe(&self) -> String {
        match self {
            Token::Ident(s) => format!("identifier `{s}`"),
            Token::Str(s) => format!("string \"{s}\""),
            Token::Int(n) => format!("integer {n}"),
            Token::At => "`@`".into(),
            Token::Plus => "`+`".into(),
            Token::Minus => "`-`".into(),
            Token::Equals => "`=`".into(),
            Token::Comma => "`,`".into(),
            Token::Colon => "`:`".into(),
            Token::Semicolon => "`;`".into(),
            Token::LParen => "`(`".into(),
            Token::RParen => "`)`".into(),
            Token::LBracket => "`[`".into(),
            Token::RBracket => "`]`".into(),
            Token::Eof => "end of input".into(),
        }
    }
}

/// A token with its byte offset, for error messages that point somewhere.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub offset: usize,
}

pub fn tokenize(src: &str) -> Result<Vec<Spanned>> {
    let b = src.as_bytes();
    let mut i = 0usize;
    let mut out = Vec::new();

    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        // `--` to end of line is a comment (SQL style).
        if c == b'-' && i + 1 < b.len() && b[i + 1] == b'-' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        let token = match c {
            b'@' => {
                i += 1;
                Token::At
            }
            b'+' => {
                i += 1;
                Token::Plus
            }
            b'-' => {
                i += 1;
                Token::Minus
            }
            b'=' => {
                i += 1;
                Token::Equals
            }
            b',' => {
                i += 1;
                Token::Comma
            }
            b':' => {
                i += 1;
                Token::Colon
            }
            b';' => {
                i += 1;
                Token::Semicolon
            }
            b'(' => {
                i += 1;
                Token::LParen
            }
            b')' => {
                i += 1;
                Token::RParen
            }
            b'[' => {
                i += 1;
                Token::LBracket
            }
            b']' => {
                i += 1;
                Token::RBracket
            }
            b'"' => {
                i += 1;
                let mut s = String::new();
                loop {
                    if i >= b.len() {
                        return Err(QError::QueryRejected(format!(
                            "unterminated string literal starting at byte {start}"
                        )));
                    }
                    match b[i] {
                        b'"' => {
                            i += 1;
                            break;
                        }
                        b'\\' => {
                            i += 1;
                            if i >= b.len() {
                                return Err(QError::QueryRejected(
                                    "trailing backslash in string literal".into(),
                                ));
                            }
                            match b[i] {
                                b'"' => s.push('"'),
                                b'\\' => s.push('\\'),
                                other => {
                                    return Err(QError::QueryRejected(format!(
                                        "unsupported escape `\\{}` — WeightQL strings support \
                                         only \\\" and \\\\",
                                        other as char
                                    )))
                                }
                            }
                            i += 1;
                        }
                        _ => {
                            let ch_start = i;
                            // Advance over one UTF-8 char.
                            i += 1;
                            while i < b.len() && (b[i] & 0xC0) == 0x80 {
                                i += 1;
                            }
                            s.push_str(&src[ch_start..i]);
                        }
                    }
                }
                Token::Str(s)
            }
            c if c.is_ascii_digit() => {
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                Token::Int(src[start..i].parse().map_err(|_| {
                    QError::QueryRejected(format!(
                        "integer literal at byte {start} is out of range"
                    ))
                })?)
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.')
                {
                    i += 1;
                }
                Token::Ident(src[start..i].to_string())
            }
            other => {
                return Err(QError::QueryRejected(format!(
                    "unexpected character `{}` at byte {start}",
                    other as char
                )))
            }
        };
        out.push(Spanned {
            token,
            offset: start,
        });
    }
    out.push(Spanned {
        token: Token::Eof,
        offset: src.len(),
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Token> {
        tokenize(src)
            .unwrap()
            .into_iter()
            .map(|s| s.token)
            .collect()
    }

    #[test]
    fn tokenizes_an_assignment_and_a_matmul() {
        assert_eq!(
            kinds(r#"A = tensor("Q[10]")"#),
            vec![
                Token::Ident("A".into()),
                Token::Equals,
                Token::Ident("tensor".into()),
                Token::LParen,
                Token::Str("Q[10]".into()),
                Token::RParen,
                Token::Eof,
            ]
        );
        assert_eq!(
            kinds("show A @ B"),
            vec![
                Token::Ident("show".into()),
                Token::Ident("A".into()),
                Token::At,
                Token::Ident("B".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn dotted_identifiers_stay_one_token() {
        assert_eq!(
            kinds("MLP.down"),
            vec![Token::Ident("MLP.down".into()), Token::Eof]
        );
    }

    #[test]
    fn ranges_and_subscripts_tokenize() {
        assert_eq!(
            kinds("[0:256, 42]"),
            vec![
                Token::LBracket,
                Token::Int(0),
                Token::Colon,
                Token::Int(256),
                Token::Comma,
                Token::Int(42),
                Token::RBracket,
                Token::Eof,
            ]
        );
    }

    #[test]
    fn comments_are_skipped() {
        assert_eq!(
            kinds("show A -- this is ignored\n@ B"),
            vec![
                Token::Ident("show".into()),
                Token::Ident("A".into()),
                Token::At,
                Token::Ident("B".into()),
                Token::Eof,
            ]
        );
    }

    #[test]
    fn string_escapes_are_limited_to_quote_and_backslash() {
        assert_eq!(
            kinds(r#""a\"b""#),
            vec![Token::Str("a\"b".into()), Token::Eof]
        );
        // No \n, \x, \u — a deliberately tiny escape set.
        assert!(tokenize(r#""a\nb""#).is_err());
    }

    #[test]
    fn unterminated_string_is_rejected() {
        assert!(tokenize(r#"tensor("Q[10]"#).is_err());
    }

    #[test]
    fn unknown_characters_are_rejected_not_ignored() {
        for bad in ["A $ B", "A | B", "A ` B", "A # B", "A & B"] {
            assert!(tokenize(bad).is_err(), "expected `{bad}` to be rejected");
        }
    }

    #[test]
    fn offsets_point_at_the_token() {
        let toks = tokenize("show  A").unwrap();
        assert_eq!(toks[0].offset, 0);
        assert_eq!(toks[1].offset, 6);
    }

    #[test]
    fn non_ascii_inside_strings_survives() {
        assert_eq!(
            kinds(r#""層[10]""#),
            vec![Token::Str("層[10]".into()), Token::Eof]
        );
    }
}
