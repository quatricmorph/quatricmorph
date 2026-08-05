//! Data plane: **Artifact Plane** configuration (ARCHITECTURE.md §2.1).
//!
//! `.plan/MEMORY_BUDGET.md` §11 — every budget is settable, in this order:
//!
//! ```text
//! CLI flag  >  environment variable  >  config file  >  compiled default
//! ```
//!
//! # Why the chain exists rather than a constant
//!
//! `.plan/MEMORY_BUDGET.md` §0: *"Every budget is a formula over a named
//! configuration variable. Not a fixed promise, because a fixed promise about
//! memory is a promise about a machine, a model, and a workload that the author
//! did not have."*
//!
//! There is a second, sharper reason, and it is why this module exists in
//! `QM-0101` rather than later. A residency **claim** needs a ceiling the claim
//! is measured *against*. If the ceiling is computed from the measured peak —
//! `C = peak / 1.25` — then "peak ≤ 1.25 × C" is an identity and can never fail,
//! so it tests nothing. `QM-0100` reported exactly that and its reviewer ruled it
//! near-tautological for exactly that reason. A ceiling has to come from
//! somewhere other than the run: a flag the operator typed, an environment
//! variable, a committed config file, or a constant compiled into the binary.
//! Those four, in that order, are what this module resolves.
//!
//! # Provenance is part of the answer
//!
//! §11 also requires that *"each is reported in the job record … so a run's
//! actual budgets are recoverable after the fact — which is what makes a
//! performance report reproducible rather than anecdotal."* So resolution does
//! not return six numbers; it returns six numbers **and where each came from**
//! ([`BudgetOrigin`]). A report that says `max_resident = 3528244` is an
//! anecdote. One that says `max_resident = 3528244 (cli-flag)` can be re-run.
//!
//! # What is refused
//!
//! | Situation | Answer |
//! | --- | --- |
//! | A config file that is not readable | [`QError::Io`] naming the path |
//! | A config file that is not valid TOML | [`QError::MalformedArtifact`] naming the path and the parse error |
//! | An unknown key under `[budgets]` | Refused naming the key **and** the accepted keys |
//! | A value of the wrong type | Refused naming the key, the expected type, and what was found |
//! | A malformed byte size in a flag, variable, or file | Refused naming the text ([`crate::budget::parse_byte_size`]) |
//! | A zero where a count must be positive | Refused naming the variable |
//!
//! **A typo is never a default.** An unrecognised key under `[budgets]` is an
//! error rather than a warning, because a budget that silently did not apply is
//! indistinguishable from a budget that was never set — and the whole value of
//! this chain is that a run's ceiling is knowable.

use crate::budget::{
    parse_byte_size, DEFAULT_BLOCK_DIMENSION, MAX_CONCURRENT_BLOCKS, MAX_HOST_STAGING_BYTES,
    MAX_OUTPUT_QUEUE_DEPTH, MAX_RESIDENT_BYTES,
};
use crate::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Where a resolved budget value came from. `.plan/MEMORY_BUDGET.md` §11's four
/// links, in precedence order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BudgetOrigin {
    CliFlag,
    Environment,
    ConfigFile,
    CompiledDefault,
}

impl BudgetOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CliFlag => "cli-flag",
            Self::Environment => "environment",
            Self::ConfigFile => "config-file",
            Self::CompiledDefault => "compiled-default",
        }
    }
}

impl std::fmt::Display for BudgetOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One budget, its value, and where the value came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedBudget {
    /// The `snake_case` variable name, identical in the config file and (upper
    /// cased, `QM_`-prefixed) in the environment.
    ///
    /// Owned rather than `&'static str` so the record deserializes: §11 wants a
    /// run's budgets *recoverable* from the job record, which means the record
    /// has to read back in, not only write out.
    pub name: String,
    pub value: u64,
    pub origin: BudgetOrigin,
}

/// How the process environment is read.
///
/// A trait rather than a direct `std::env::var` call because tests must be able
/// to exercise the environment link of the chain **without mutating the process
/// environment**. `std::env::set_var` is process-global and `cargo test` runs
/// tests on many threads, so a test that set a variable would perturb every
/// other test in the binary — and the resulting flake would look like a
/// precedence bug rather than a harness bug.
pub trait EnvLookup {
    fn get(&self, key: &str) -> Option<String>;
}

/// The real environment.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessEnv;

impl EnvLookup for ProcessEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

impl EnvLookup for BTreeMap<String, String> {
    fn get(&self, key: &str) -> Option<String> {
        BTreeMap::get(self, key).cloned()
    }
}

/// Nothing set. Used when a caller wants the chain without its environment link.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyEnv;

impl EnvLookup for EmptyEnv {
    fn get(&self, _key: &str) -> Option<String> {
        None
    }
}

/// The highest-precedence link: what a CLI parsed from its flags.
///
/// Every field is `Option` because "not given" and "given as the default value"
/// must stay distinguishable — otherwise the reported origin would be wrong for
/// an operator who explicitly typed the default.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BudgetFlags {
    pub max_resident_bytes: Option<u64>,
    pub max_host_staging_bytes: Option<u64>,
    pub max_concurrent_blocks: Option<u64>,
    pub max_output_queue_depth: Option<u64>,
    pub block_rows: Option<u64>,
    pub block_columns: Option<u64>,
}

/// The six streaming budgets of `.plan/MEMORY_BUDGET.md` §4 and §11, resolved
/// with provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamingBudgets {
    pub max_resident_bytes: ResolvedBudget,
    pub max_host_staging_bytes: ResolvedBudget,
    pub max_concurrent_blocks: ResolvedBudget,
    pub max_output_queue_depth: ResolvedBudget,
    pub block_rows: ResolvedBudget,
    pub block_columns: ResolvedBudget,
}

/// Whether a variable is a byte count (and so accepts `64MiB`) or a plain count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unit {
    Bytes,
    Count,
}

/// One row of the resolution table: the variable, its unit, its compiled
/// default, and how to pull it out of [`BudgetFlags`].
struct Variable {
    name: &'static str,
    unit: Unit,
    compiled_default: u64,
    flag: fn(&BudgetFlags) -> Option<u64>,
}

/// Every variable this chain resolves. The table is the single source of the
/// accepted key set, so the config-file validator, the environment reader, and
/// the error message that lists valid keys cannot drift apart.
const VARIABLES: &[Variable] = &[
    Variable {
        name: "max_resident_bytes",
        unit: Unit::Bytes,
        compiled_default: MAX_RESIDENT_BYTES,
        flag: |f| f.max_resident_bytes,
    },
    Variable {
        name: "max_host_staging_bytes",
        unit: Unit::Bytes,
        compiled_default: MAX_HOST_STAGING_BYTES,
        flag: |f| f.max_host_staging_bytes,
    },
    Variable {
        name: "max_concurrent_blocks",
        unit: Unit::Count,
        compiled_default: MAX_CONCURRENT_BLOCKS as u64,
        flag: |f| f.max_concurrent_blocks,
    },
    Variable {
        name: "max_output_queue_depth",
        unit: Unit::Count,
        compiled_default: MAX_OUTPUT_QUEUE_DEPTH as u64,
        flag: |f| f.max_output_queue_depth,
    },
    Variable {
        name: "block_rows",
        unit: Unit::Count,
        compiled_default: DEFAULT_BLOCK_DIMENSION,
        flag: |f| f.block_rows,
    },
    Variable {
        name: "block_columns",
        unit: Unit::Count,
        compiled_default: DEFAULT_BLOCK_DIMENSION,
        flag: |f| f.block_columns,
    },
];

/// The environment variable name for `variable`: `QM_` + upper case.
/// `.plan/MEMORY_BUDGET.md` §11 names two of these literally
/// (`QM_MAX_HOST_STAGING_BYTES`, `QM_MAX_CONCURRENT_BLOCKS`) "and so on", so the
/// rule is derived from those rather than tabulated separately.
pub fn environment_variable_name(variable: &str) -> String {
    format!("QM_{}", variable.to_ascii_uppercase())
}

/// The keys accepted under `[budgets]`, for an error message that tells the
/// operator what they may write instead of only that they were wrong.
pub fn accepted_budget_keys() -> Vec<&'static str> {
    VARIABLES.iter().map(|v| v.name).collect()
}

fn parse_value(unit: Unit, name: &str, origin: BudgetOrigin, text: &str) -> Result<u64> {
    let value = match unit {
        Unit::Bytes => parse_byte_size(text).map_err(|e| {
            QError::QueryRejected(format!("budget {name} from {origin}: {}", plain(&e)))
        })?,
        Unit::Count => text.trim().parse::<u64>().map_err(|_| {
            QError::QueryRejected(format!(
                "budget {name} from {origin}: `{text}` is not a count. This variable is a \
                 number of blocks or rows, not a byte size, so it takes a bare decimal \
                 integer"
            ))
        })?,
    };
    if value == 0 {
        return Err(QError::QueryRejected(format!(
            "budget {name} from {origin}: 0 is not a usable budget. A zero ceiling admits \
             nothing and a zero count cannot make progress; refuse rather than silently \
             substituting a default"
        )));
    }
    Ok(value)
}

/// `QError`'s `Display` already prefixes its variant; strip nothing, just render.
fn plain(e: &QError) -> String {
    e.to_string()
}

/// The `[budgets]` table of a config file, validated key by key.
///
/// Values are read as TOML values rather than typed fields so that a byte size
/// may be written either as an integer (`max_resident_bytes = 3528244`) or as a
/// string (`max_resident_bytes = "64MiB"`), which is how an operator will
/// actually want to write it.
fn read_config_file(path: &Path) -> Result<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path).map_err(|e| QError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let parsed: toml::Value = toml::from_str(&text).map_err(|e| {
        QError::malformed(
            path.to_string_lossy(),
            format!("not valid TOML: {e}. A budget config file is a `[budgets]` table"),
        )
    })?;
    let table = match parsed.get("budgets") {
        None => return Ok(BTreeMap::new()),
        Some(toml::Value::Table(t)) => t,
        Some(other) => {
            return Err(QError::malformed(
                path.to_string_lossy(),
                format!(
                    "`budgets` must be a table, found {}. Write it as `[budgets]`",
                    other.type_str()
                ),
            ))
        }
    };
    let mut out = BTreeMap::new();
    for (key, value) in table {
        if !VARIABLES.iter().any(|v| v.name == key.as_str()) {
            return Err(QError::malformed(
                path.to_string_lossy(),
                format!(
                    "`budgets.{key}` is not a budget. A key that does not apply is refused \
                     rather than ignored, because a budget that silently did not apply \
                     cannot be told from one that was never set. Accepted keys: {}",
                    accepted_budget_keys().join(", ")
                ),
            ));
        }
        let rendered = match value {
            toml::Value::Integer(i) if *i >= 0 => i.to_string(),
            toml::Value::String(s) => s.clone(),
            other => {
                return Err(QError::malformed(
                    path.to_string_lossy(),
                    format!(
                        "`budgets.{key}` must be a non-negative integer or a string like \
                         \"64MiB\", found {}",
                        other.type_str()
                    ),
                ))
            }
        };
        out.insert(key.clone(), rendered);
    }
    Ok(out)
}

impl StreamingBudgets {
    /// The compiled defaults, with every origin reported as such.
    pub fn compiled_defaults() -> Self {
        Self::resolve(&BudgetFlags::default(), &EmptyEnv, None)
            .expect("the compiled defaults are valid by construction")
    }

    /// Walk `.plan/MEMORY_BUDGET.md` §11's chain for all six variables.
    ///
    /// `config_path` is consulted only when given; an absent path is not an
    /// error, but a path that is given and cannot be read **is** — a config file
    /// the operator named and this code silently skipped would produce a run
    /// whose budgets are not the ones asked for.
    pub fn resolve(
        flags: &BudgetFlags,
        env: &dyn EnvLookup,
        config_path: Option<&Path>,
    ) -> Result<Self> {
        let from_file = match config_path {
            Some(path) => read_config_file(path)?,
            None => BTreeMap::new(),
        };

        let mut resolved: BTreeMap<&'static str, ResolvedBudget> = BTreeMap::new();
        for variable in VARIABLES {
            let budget = if let Some(value) = (variable.flag)(flags) {
                // A flag arrives already parsed, but the zero check belongs to
                // every link of the chain rather than only to the parsed ones.
                if value == 0 {
                    return Err(QError::QueryRejected(format!(
                        "budget {} from {}: 0 is not a usable budget",
                        variable.name,
                        BudgetOrigin::CliFlag
                    )));
                }
                ResolvedBudget {
                    name: variable.name.to_string(),
                    value,
                    origin: BudgetOrigin::CliFlag,
                }
            } else if let Some(text) = env.get(&environment_variable_name(variable.name)) {
                ResolvedBudget {
                    name: variable.name.to_string(),
                    value: parse_value(
                        variable.unit,
                        variable.name,
                        BudgetOrigin::Environment,
                        &text,
                    )?,
                    origin: BudgetOrigin::Environment,
                }
            } else if let Some(text) = from_file.get(variable.name) {
                ResolvedBudget {
                    name: variable.name.to_string(),
                    value: parse_value(
                        variable.unit,
                        variable.name,
                        BudgetOrigin::ConfigFile,
                        text,
                    )?,
                    origin: BudgetOrigin::ConfigFile,
                }
            } else {
                ResolvedBudget {
                    name: variable.name.to_string(),
                    value: variable.compiled_default,
                    origin: BudgetOrigin::CompiledDefault,
                }
            };
            resolved.insert(variable.name, budget);
        }

        let take = |name: &str| -> ResolvedBudget {
            resolved
                .get(name)
                .cloned()
                .expect("VARIABLES covers every field of StreamingBudgets")
        };
        Ok(Self {
            max_resident_bytes: take("max_resident_bytes"),
            max_host_staging_bytes: take("max_host_staging_bytes"),
            max_concurrent_blocks: take("max_concurrent_blocks"),
            max_output_queue_depth: take("max_output_queue_depth"),
            block_rows: take("block_rows"),
            block_columns: take("block_columns"),
        })
    }

    /// Every resolved budget, in the declaration order of the resolution table.
    pub fn all(&self) -> Vec<&ResolvedBudget> {
        vec![
            &self.max_resident_bytes,
            &self.max_host_staging_bytes,
            &self.max_concurrent_blocks,
            &self.max_output_queue_depth,
            &self.block_rows,
            &self.block_columns,
        ]
    }

    /// `usize` view of a count, clamped at the platform width.
    pub fn concurrent_blocks(&self) -> usize {
        usize::try_from(self.max_concurrent_blocks.value).unwrap_or(usize::MAX)
    }

    pub fn output_queue_depth(&self) -> usize {
        usize::try_from(self.max_output_queue_depth.value).unwrap_or(usize::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::MAX_RESIDENT_BYTES;

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn write(dir: &Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("quatricmorph.toml");
        std::fs::write(&path, body).expect("write the config fixture");
        path
    }

    #[test]
    fn the_compiled_default_resident_ceiling_is_two_gibibytes_and_reports_itself_as_compiled() {
        let b = StreamingBudgets::compiled_defaults();
        assert_eq!(b.max_resident_bytes.value, 2 * 1024 * 1024 * 1024);
        assert_eq!(b.max_resident_bytes.value, MAX_RESIDENT_BYTES);
        for budget in b.all() {
            assert_eq!(
                budget.origin,
                BudgetOrigin::CompiledDefault,
                "{} reported {}",
                budget.name,
                budget.origin
            );
        }
        // The other five defaults are `.plan/MEMORY_BUDGET.md` §4's table.
        assert_eq!(b.max_host_staging_bytes.value, 512 * 1024 * 1024);
        assert_eq!(b.max_concurrent_blocks.value, 4);
        assert_eq!(b.max_output_queue_depth.value, 64);
        assert_eq!((b.block_rows.value, b.block_columns.value), (256, 256));
    }

    /// `.plan/MEMORY_BUDGET.md` §11: `CLI flag > environment > config file >
    /// compiled default`. All four links are populated with *different* values
    /// so the winner identifies which link won, rather than merely that some
    /// value arrived.
    #[test]
    fn a_cli_flag_beats_the_environment_which_beats_the_config_file_which_beats_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(
            dir.path(),
            "[budgets]\n\
             max_resident_bytes = 3000\n\
             max_host_staging_bytes = 4000\n\
             max_concurrent_blocks = 5\n",
        );
        let environment = env(&[
            ("QM_MAX_RESIDENT_BYTES", "2000"),
            ("QM_MAX_HOST_STAGING_BYTES", "2500"),
        ]);
        let flags = BudgetFlags {
            max_resident_bytes: Some(1000),
            ..Default::default()
        };

        let b = StreamingBudgets::resolve(&flags, &environment, Some(&path)).unwrap();

        // Flag wins over an environment variable and a file that both set it.
        assert_eq!(b.max_resident_bytes.value, 1000);
        assert_eq!(b.max_resident_bytes.origin, BudgetOrigin::CliFlag);
        // Environment wins over the file.
        assert_eq!(b.max_host_staging_bytes.value, 2500);
        assert_eq!(b.max_host_staging_bytes.origin, BudgetOrigin::Environment);
        // The file wins over the compiled default.
        assert_eq!(b.max_concurrent_blocks.value, 5);
        assert_eq!(b.max_concurrent_blocks.origin, BudgetOrigin::ConfigFile);
        // Untouched by any link.
        assert_eq!(b.block_rows.value, 256);
        assert_eq!(b.block_rows.origin, BudgetOrigin::CompiledDefault);
    }

    #[test]
    fn a_byte_valued_budget_accepts_binary_and_decimal_units_from_every_link() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[budgets]\nmax_resident_bytes = \"2GiB\"\n");
        let b = StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&path)).unwrap();
        assert_eq!(b.max_resident_bytes.value, 2 * 1024 * 1024 * 1024);
        assert_eq!(b.max_resident_bytes.origin, BudgetOrigin::ConfigFile);

        let b = StreamingBudgets::resolve(
            &BudgetFlags::default(),
            &env(&[("QM_MAX_RESIDENT_BYTES", "64MiB")]),
            None,
        )
        .unwrap();
        assert_eq!(b.max_resident_bytes.value, 64 * 1024 * 1024);

        // `GB` is 10^9 and `GiB` is 2^30; conflating them moves a ceiling by 7 %.
        let b = StreamingBudgets::resolve(
            &BudgetFlags::default(),
            &env(&[("QM_MAX_RESIDENT_BYTES", "2GB")]),
            None,
        )
        .unwrap();
        assert_eq!(b.max_resident_bytes.value, 2_000_000_000);
        assert_ne!(b.max_resident_bytes.value, 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn a_malformed_byte_size_is_refused_naming_the_variable_and_the_text() {
        let err = StreamingBudgets::resolve(
            &BudgetFlags::default(),
            &env(&[("QM_MAX_RESIDENT_BYTES", "two gigabytes")]),
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("max_resident_bytes"), "message was {msg}");
        assert!(msg.contains("environment"), "message was {msg}");
        assert!(msg.contains("two gigabytes"), "message was {msg}");
    }

    #[test]
    fn a_count_valued_budget_refuses_a_byte_size_rather_than_reading_it_as_a_count() {
        // `max_concurrent_blocks = 4MiB` is a category error: 4 194 304
        // concurrent decoded blocks is not what anyone means, so the unit is
        // refused rather than silently multiplied.
        let err = StreamingBudgets::resolve(
            &BudgetFlags::default(),
            &env(&[("QM_MAX_CONCURRENT_BLOCKS", "4MiB")]),
            None,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("max_concurrent_blocks"), "message was {msg}");
        assert!(msg.contains("not a count"), "message was {msg}");
    }

    #[test]
    fn a_zero_budget_is_refused_from_every_link_rather_than_falling_back_to_the_default() {
        for (flags, environment) in [
            (
                BudgetFlags {
                    max_resident_bytes: Some(0),
                    ..Default::default()
                },
                BTreeMap::new(),
            ),
            (
                BudgetFlags::default(),
                env(&[("QM_MAX_RESIDENT_BYTES", "0")]),
            ),
        ] {
            let err = StreamingBudgets::resolve(&flags, &environment, None).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("max_resident_bytes"), "message was {msg}");
            assert!(msg.contains('0'), "message was {msg}");
        }
    }

    #[test]
    fn an_unknown_budget_key_is_refused_naming_the_accepted_keys() {
        let dir = tempfile::tempdir().unwrap();
        // A plausible typo: the real key is `max_resident_bytes`.
        let path = write(dir.path(), "[budgets]\nmax_resident_byte = 4096\n");
        let err =
            StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&path)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("max_resident_byte"), "message was {msg}");
        assert!(msg.contains("max_resident_bytes"), "message was {msg}");
        assert!(
            msg.contains("refused rather than ignored"),
            "message was {msg}"
        );
    }

    #[test]
    fn a_config_file_that_is_not_valid_toml_is_refused_naming_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[budgets\nmax_resident_bytes = 4096\n");
        let err =
            StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&path)).unwrap_err();
        assert!(
            matches!(err, QError::MalformedArtifact { .. }),
            "expected MalformedArtifact, got {err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("quatricmorph.toml"), "message was {msg}");
        assert!(msg.contains("not valid TOML"), "message was {msg}");
    }

    #[test]
    fn a_config_file_that_was_named_but_does_not_exist_is_refused_never_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("absent.toml");
        let err = StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&missing))
            .unwrap_err();
        assert!(matches!(err, QError::Io { .. }), "got {err:?}");
        assert!(err.to_string().contains("absent.toml"), "message was {err}");
    }

    #[test]
    fn a_config_file_with_no_budgets_table_resolves_to_the_compiled_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "# nothing to say about budgets\n");
        let b = StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&path)).unwrap();
        assert_eq!(b.max_resident_bytes.value, MAX_RESIDENT_BYTES);
        assert_eq!(b.max_resident_bytes.origin, BudgetOrigin::CompiledDefault);
    }

    #[test]
    fn budgets_that_are_not_a_table_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "budgets = 4096\n");
        let err =
            StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&path)).unwrap_err();
        assert!(
            err.to_string().contains("must be a table"),
            "message was {err}"
        );
    }

    #[test]
    fn a_budget_value_of_the_wrong_toml_type_is_refused_naming_the_key_and_the_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), "[budgets]\nmax_resident_bytes = true\n");
        let err =
            StreamingBudgets::resolve(&BudgetFlags::default(), &EmptyEnv, Some(&path)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("max_resident_bytes"), "message was {msg}");
        assert!(msg.contains("boolean"), "message was {msg}");
    }

    /// `.plan/MEMORY_BUDGET.md` §11 names `QM_MAX_HOST_STAGING_BYTES` and
    /// `QM_MAX_CONCURRENT_BLOCKS` literally. The naming rule must reproduce the
    /// document's own examples, not merely be self-consistent.
    #[test]
    fn environment_names_match_the_two_the_memory_budget_document_spells_out() {
        assert_eq!(
            environment_variable_name("max_host_staging_bytes"),
            "QM_MAX_HOST_STAGING_BYTES"
        );
        assert_eq!(
            environment_variable_name("max_concurrent_blocks"),
            "QM_MAX_CONCURRENT_BLOCKS"
        );
        assert_eq!(
            environment_variable_name("max_resident_bytes"),
            "QM_MAX_RESIDENT_BYTES"
        );
        assert_eq!(
            accepted_budget_keys(),
            vec![
                "max_resident_bytes",
                "max_host_staging_bytes",
                "max_concurrent_blocks",
                "max_output_queue_depth",
                "block_rows",
                "block_columns",
            ]
        );
    }

    /// §11's reason for the chain: *"a run's actual budgets are recoverable
    /// after the fact"*. That requires the provenance to survive serialization,
    /// not only to exist in memory.
    #[test]
    fn every_resolved_budget_serializes_with_its_origin_so_a_run_is_reproducible() {
        let b = StreamingBudgets::resolve(
            &BudgetFlags {
                block_rows: Some(128),
                ..Default::default()
            },
            &env(&[("QM_BLOCK_COLUMNS", "128")]),
            None,
        )
        .unwrap();
        let wire = serde_json::to_value(&b).unwrap();
        assert_eq!(wire["block_rows"]["value"], serde_json::json!(128));
        assert_eq!(wire["block_rows"]["origin"], serde_json::json!("cli-flag"));
        assert_eq!(
            wire["block_columns"]["origin"],
            serde_json::json!("environment")
        );
        assert_eq!(
            wire["max_resident_bytes"]["origin"],
            serde_json::json!("compiled-default")
        );
        // And it round-trips, so a recorded run can be replayed.
        let back: StreamingBudgets = serde_json::from_value(wire).unwrap();
        assert_eq!(back, b);
    }
}
