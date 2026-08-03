//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §7).
//!
//! WeightQL parser: tokens → statements.
//!
//! ## The grammar implemented in this pass
//!
//! ```text
//! script      := statement (';'? statement)* ';'?
//! statement   := assignment | show | select_value | select_slice
//! assignment  := IDENT '=' expr
//! show        := 'show' expr
//! select_value:= 'SELECT' 'value' 'FROM' tensor_call 'AT' '[' int (',' int)* ']'
//! select_slice:= 'SELECT' 'slice' 'FROM' tensor_call
//!                ['ROWS' range] ['COLUMNS' range]
//!
//! expr        := add
//! add         := mul (('+'|'-') mul)*
//! mul         := postfix ('@' postfix)*
//! postfix     := primary ('[' selector ']')*
//! primary     := tensor_call | call | IDENT | '(' expr ')'
//! tensor_call := 'tensor' '(' STRING ')'
//! call        := ('transpose'|'min'|'max'|'mean'|'variance'|'stddev'
//!                |'l1_norm'|'l2_norm'|'zero_ratio') '(' expr ')'
//!              | 'compare' '(' expr ',' expr ')' 'by' IDENT
//! ```
//!
//! Keywords are matched case-insensitively so both the SQL-flavoured forms of
//! ARCHITECTURE.md §7.1–7.3 and the expression form of §7.4 read naturally.
//!
//! There is no production for `eval`, for a function *definition*, for a shell
//! escape, or for raw SQL. Adding one would require changing
//! [`q_expression::Expr`], which is a closed enum.

use crate::lexer::{tokenize, Spanned, Token};
use q_expression::{ComparisonMetric, Expr, Reduction};
use q_nsir::{ElementSelector, IndexTerm};
use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};

/// One WeightQL statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    /// `A = tensor("Q[10]")`
    Assign { name: String, expr: Expr },
    /// `show A @ B`
    Show(Expr),
    /// `SELECT value FROM tensor("…") AT [100, 42];`
    SelectValue { reference: String, index: Vec<u64> },
    /// `SELECT slice FROM tensor("Q[10]") ROWS 0:256 COLUMNS 0:256;`
    SelectSlice {
        reference: String,
        rows: Option<(Option<u64>, Option<u64>)>,
        columns: Option<(Option<u64>, Option<u64>)>,
    },
}

/// A parsed script: an ordered list of statements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Script {
    pub statements: Vec<Statement>,
}

impl Script {
    /// The statement whose result the caller wants, if any (`show` / `SELECT`).
    pub fn output_statement(&self) -> Option<&Statement> {
        self.statements
            .iter()
            .rev()
            .find(|s| !matches!(s, Statement::Assign { .. }))
    }
}

pub fn parse(src: &str) -> Result<Script> {
    let tokens = tokenize(src)?;
    let mut p = Parser { tokens, pos: 0 };
    let script = p.script()?;
    p.expect(Token::Eof)?;
    if script.statements.is_empty() {
        return Err(QError::QueryRejected("empty query".into()));
    }
    Ok(script)
}

struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)].token
    }

    fn offset(&self) -> usize {
        self.tokens[self.pos.min(self.tokens.len() - 1)].offset
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens[self.pos.min(self.tokens.len() - 1)].token.clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn reject(&self, what: impl std::fmt::Display) -> QError {
        QError::QueryRejected(format!(
            "{what} at byte {}, found {}",
            self.offset(),
            self.peek().describe()
        ))
    }

    fn expect(&mut self, want: Token) -> Result<()> {
        if *self.peek() == want {
            self.bump();
            Ok(())
        } else {
            Err(self.reject(format!("expected {}", want.describe())))
        }
    }

    /// Case-insensitive keyword check.
    fn at_keyword(&self, kw: &str) -> bool {
        matches!(self.peek(), Token::Ident(s) if s.eq_ignore_ascii_case(kw))
    }

    fn eat_keyword(&mut self, kw: &str) -> bool {
        if self.at_keyword(kw) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_keyword(&mut self, kw: &str) -> Result<()> {
        if self.eat_keyword(kw) {
            Ok(())
        } else {
            Err(self.reject(format!("expected keyword `{kw}`")))
        }
    }

    fn ident(&mut self) -> Result<String> {
        match self.peek().clone() {
            Token::Ident(s) => {
                self.bump();
                Ok(s)
            }
            _ => Err(self.reject("expected an identifier")),
        }
    }

    fn integer(&mut self) -> Result<u64> {
        match *self.peek() {
            Token::Int(n) => {
                self.bump();
                Ok(n)
            }
            _ => Err(self.reject("expected an integer")),
        }
    }

    fn script(&mut self) -> Result<Script> {
        let mut statements = Vec::new();
        loop {
            while *self.peek() == Token::Semicolon {
                self.bump();
            }
            if *self.peek() == Token::Eof {
                break;
            }
            statements.push(self.statement()?);
        }
        Ok(Script { statements })
    }

    fn statement(&mut self) -> Result<Statement> {
        if self.at_keyword("select") {
            return self.select_statement();
        }
        if self.eat_keyword("show") {
            return Ok(Statement::Show(self.expr()?));
        }
        // `NAME = expr` — lookahead for `=`.
        if let Token::Ident(name) = self.peek().clone() {
            if self.tokens.get(self.pos + 1).map(|s| &s.token) == Some(&Token::Equals) {
                self.bump();
                self.bump();
                return Ok(Statement::Assign {
                    name,
                    expr: self.expr()?,
                });
            }
        }
        Err(self.reject(
            "expected a statement: `NAME = expr`, `show expr`, or `SELECT value|slice FROM …`",
        ))
    }

    fn select_statement(&mut self) -> Result<Statement> {
        self.expect_keyword("select")?;
        let what = self.ident()?.to_ascii_lowercase();
        self.expect_keyword("from")?;
        let reference = self.tensor_call_string()?;

        match what.as_str() {
            "value" => {
                self.expect_keyword("at")?;
                self.expect(Token::LBracket)?;
                let mut index = vec![self.integer()?];
                while *self.peek() == Token::Comma {
                    self.bump();
                    index.push(self.integer()?);
                }
                self.expect(Token::RBracket)?;
                Ok(Statement::SelectValue { reference, index })
            }
            "slice" => {
                let mut rows = None;
                let mut columns = None;
                if self.eat_keyword("rows") {
                    rows = Some(self.open_range()?);
                }
                if self.eat_keyword("columns") {
                    columns = Some(self.open_range()?);
                }
                Ok(Statement::SelectSlice {
                    reference,
                    rows,
                    columns,
                })
            }
            other => Err(QError::QueryRejected(format!(
                "SELECT {other} is not supported; this pass implements `SELECT value` and \
                 `SELECT slice` (ARCHITECTURE.md §7.1–7.2). Statistical SELECT is WQL-007."
            ))),
        }
    }

    /// `a:b`, `:b`, `a:` — the `ROWS`/`COLUMNS` form.
    fn open_range(&mut self) -> Result<(Option<u64>, Option<u64>)> {
        let start = if matches!(self.peek(), Token::Int(_)) {
            Some(self.integer()?)
        } else {
            None
        };
        self.expect(Token::Colon)?;
        let end = if matches!(self.peek(), Token::Int(_)) {
            Some(self.integer()?)
        } else {
            None
        };
        if start.is_none() && end.is_none() {
            return Err(QError::QueryRejected("range `:` needs a bound".into()));
        }
        Ok((start, end))
    }

    fn tensor_call_string(&mut self) -> Result<String> {
        self.expect_keyword("tensor")?;
        self.expect(Token::LParen)?;
        let s = match self.peek().clone() {
            Token::Str(s) => {
                self.bump();
                s
            }
            _ => return Err(self.reject("expected a quoted tensor address or alias")),
        };
        self.expect(Token::RParen)?;
        Ok(s)
    }

    fn expr(&mut self) -> Result<Expr> {
        self.add()
    }

    fn add(&mut self) -> Result<Expr> {
        let mut left = self.mul()?;
        loop {
            match self.peek() {
                Token::Plus => {
                    self.bump();
                    left = Expr::Add(Box::new(left), Box::new(self.mul()?));
                }
                Token::Minus => {
                    self.bump();
                    left = Expr::Sub(Box::new(left), Box::new(self.mul()?));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn mul(&mut self) -> Result<Expr> {
        let mut left = self.postfix()?;
        while *self.peek() == Token::At {
            self.bump();
            left = Expr::MatMul(Box::new(left), Box::new(self.postfix()?));
        }
        Ok(left)
    }

    fn postfix(&mut self) -> Result<Expr> {
        let mut e = self.primary()?;
        while *self.peek() == Token::LBracket {
            let selector = self.selector()?;
            e = Expr::Slice {
                operand: Box::new(e),
                selector,
            };
        }
        Ok(e)
    }

    fn selector(&mut self) -> Result<ElementSelector> {
        self.expect(Token::LBracket)?;
        let mut terms = Vec::new();
        loop {
            let term = if *self.peek() == Token::Colon {
                self.bump();
                if let Token::Int(n) = *self.peek() {
                    self.bump();
                    IndexTerm::Range {
                        start: None,
                        end: Some(n),
                    }
                } else {
                    IndexTerm::All
                }
            } else {
                let n = self.integer()?;
                if *self.peek() == Token::Colon {
                    self.bump();
                    if let Token::Int(m) = *self.peek() {
                        self.bump();
                        IndexTerm::Range {
                            start: Some(n),
                            end: Some(m),
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
            if *self.peek() == Token::Comma {
                self.bump();
                continue;
            }
            break;
        }
        self.expect(Token::RBracket)?;
        Ok(ElementSelector(terms))
    }

    fn primary(&mut self) -> Result<Expr> {
        if *self.peek() == Token::LParen {
            self.bump();
            let e = self.expr()?;
            self.expect(Token::RParen)?;
            return Ok(e);
        }
        let name = match self.peek().clone() {
            Token::Ident(s) => s,
            _ => return Err(self.reject("expected a tensor reference or function call")),
        };
        let lower = name.to_ascii_lowercase();

        if lower == "tensor" {
            return Ok(Expr::tensor(self.tensor_call_string()?));
        }
        if lower == "transpose" {
            self.bump();
            self.expect(Token::LParen)?;
            let inner = self.expr()?;
            self.expect(Token::RParen)?;
            return Ok(Expr::Transpose(Box::new(inner)));
        }
        if lower == "compare" {
            self.bump();
            self.expect(Token::LParen)?;
            let left = self.expr()?;
            self.expect(Token::Comma)?;
            let right = self.expr()?;
            self.expect(Token::RParen)?;
            self.expect_keyword("by")?;
            let metric_name = self.ident()?;
            let metric = ComparisonMetric::parse(&metric_name.to_ascii_lowercase()).ok_or_else(
                || {
                    QError::QueryRejected(format!(
                        "unknown comparison metric `{metric_name}`; supported: \
                         cosine_similarity, relative_l2"
                    ))
                },
            )?;
            return Ok(Expr::Compare {
                left: Box::new(left),
                right: Box::new(right),
                metric,
            });
        }
        if let Some(reduction) = Reduction::parse(&lower) {
            // Only treat it as a reduction if it is actually called.
            if self.tokens.get(self.pos + 1).map(|s| &s.token) == Some(&Token::LParen) {
                self.bump();
                self.expect(Token::LParen)?;
                let inner = self.expr()?;
                self.expect(Token::RParen)?;
                return Ok(Expr::Reduce {
                    reduction,
                    operand: Box::new(inner),
                });
            }
        }
        // A bare identifier: a previously bound name.
        if self.tokens.get(self.pos + 1).map(|s| &s.token) == Some(&Token::LParen) {
            return Err(QError::QueryRejected(format!(
                "unknown function `{name}`. WeightQL has a fixed function set: tensor, \
                 transpose, compare, min, max, mean, variance, stddev, l1_norm, l2_norm, \
                 zero_ratio. There is no `eval` and no way to define new functions."
            )));
        }
        self.bump();
        Ok(Expr::binding(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_architecture_md_matmul_script() {
        let src = r#"
            A = tensor("Q[10][0:256,0:256]")
            B = transpose(tensor("K[10][0:256,0:256]"))
            show A @ B
        "#;
        let script = parse(src).unwrap();
        assert_eq!(script.statements.len(), 3);

        // The parser produces the tree as written: bindings stay bindings.
        // Substituting `B` -> `transpose(tensor("K[10][…]"))` is the planner's
        // job (plan::substitute), so that shape checking sees the real operand.
        match &script.statements[1] {
            Statement::Assign { name, expr } => {
                assert_eq!(name, "B");
                assert!(matches!(expr, Expr::Transpose(_)));
            }
            other => panic!("expected `B = transpose(...)`, got {other:?}"),
        }
        match &script.statements[2] {
            Statement::Show(Expr::MatMul(a, b)) => {
                assert_eq!(**a, Expr::binding("A"));
                assert_eq!(**b, Expr::binding("B"));
            }
            other => panic!("expected MatMul(A, B), got {other:?}"),
        }
    }

    #[test]
    fn matmul_is_left_associative_giving_the_documented_tree() {
        // ARCHITECTURE.md §7.4: (A @ B) @ C
        let script = parse("show A @ B @ C").unwrap();
        match script.output_statement().unwrap() {
            Statement::Show(e) => {
                assert_eq!(e.to_string(), "((A @ B) @ C)");
                assert_eq!(e.matmul_count(), 2);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parentheses_override_associativity() {
        let script = parse("show A @ (B @ C)").unwrap();
        match script.output_statement().unwrap() {
            Statement::Show(e) => assert_eq!(e.to_string(), "(A @ (B @ C))"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_the_sql_scalar_form() {
        let src = r#"SELECT value
                     FROM tensor("model.layers.10.self_attn.q_proj.weight")
                     AT [100, 42];"#;
        match &parse(src).unwrap().statements[0] {
            Statement::SelectValue { reference, index } => {
                assert_eq!(reference, "model.layers.10.self_attn.q_proj.weight");
                assert_eq!(index, &vec![100, 42]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_the_sql_slice_form() {
        let src = r#"SELECT slice FROM tensor("Q[10]") ROWS 0:256 COLUMNS 0:256;"#;
        match &parse(src).unwrap().statements[0] {
            Statement::SelectSlice {
                reference,
                rows,
                columns,
            } => {
                assert_eq!(reference, "Q[10]");
                assert_eq!(*rows, Some((Some(0), Some(256))));
                assert_eq!(*columns, Some((Some(0), Some(256))));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn keywords_are_case_insensitive() {
        assert!(parse(r#"select VALUE from TENSOR("Q[10]") at [1,2]"#).is_ok());
        assert!(parse(r#"SHOW A @ B"#).is_ok());
    }

    #[test]
    fn postfix_slices_attach_to_expressions() {
        let script = parse("show A[0:128, :]").unwrap();
        match script.output_statement().unwrap() {
            Statement::Show(Expr::Slice { selector, .. }) => {
                assert_eq!(
                    selector.0,
                    vec![
                        IndexTerm::Range { start: Some(0), end: Some(128) },
                        IndexTerm::All
                    ]
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn reductions_and_comparisons_parse() {
        let s = parse("show l2_norm(A)").unwrap();
        assert!(matches!(
            s.output_statement().unwrap(),
            Statement::Show(Expr::Reduce {
                reduction: Reduction::L2Norm,
                ..
            })
        ));
        let c = parse("show compare(A, B) by cosine_similarity").unwrap();
        assert!(matches!(
            c.output_statement().unwrap(),
            Statement::Show(Expr::Compare {
                metric: ComparisonMetric::CosineSimilarity,
                ..
            })
        ));
    }

    #[test]
    fn arbitrary_code_execution_constructs_are_rejected() {
        // These are the forms §11 hard-bans. Each must fail to parse.
        for hostile in [
            r#"show eval("1+1")"#,
            r#"A = Function("return 1")"#,
            r#"show system("rm -rf /")"#,
            r#"show exec("cat /etc/passwd")"#,
            r#"SELECT * FROM sqlite_master"#,
            r#"show require("fs")"#,
        ] {
            let err = parse(hostile).unwrap_err();
            assert!(
                matches!(err, QError::QueryRejected(_)),
                "expected `{hostile}` to be rejected, got {err:?}"
            );
        }
    }

    #[test]
    fn unknown_function_error_names_the_closed_function_set() {
        let err = parse(r#"show eval("x")"#).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("fixed function set"), "{msg}");
        assert!(msg.contains("no `eval`"), "{msg}");
    }

    #[test]
    fn malformed_input_is_rejected_with_position() {
        for bad in [
            "show A @",
            "show (A @ B",
            "A =",
            "= tensor(\"x\")",
            "show A[0:",
            "SELECT value FROM tensor(\"x\")",
            "",
            "   ",
            ";;;",
        ] {
            assert!(parse(bad).is_err(), "expected `{bad}` to be rejected");
        }
        let err = parse("show A @").unwrap_err();
        assert!(err.to_string().contains("byte"), "{err}");
    }

    #[test]
    fn statements_may_be_separated_by_semicolons_or_newlines() {
        assert_eq!(parse("A = tensor(\"x\"); show A").unwrap().statements.len(), 2);
        assert_eq!(parse("A = tensor(\"x\")\nshow A").unwrap().statements.len(), 2);
    }

    #[test]
    fn unsupported_select_target_is_named_with_its_requirement() {
        let err = parse(r#"SELECT mean FROM tensor("Q[10]")"#).unwrap_err();
        assert!(err.to_string().contains("WQL-007"), "{err}");
    }

    #[test]
    fn output_statement_is_the_last_non_assignment() {
        let s = parse("A = tensor(\"x\")\nB = tensor(\"y\")\nshow A @ B").unwrap();
        assert!(matches!(s.output_statement(), Some(Statement::Show(_))));
        let only_assign = parse("A = tensor(\"x\")").unwrap();
        assert!(only_assign.output_statement().is_none());
    }
}
