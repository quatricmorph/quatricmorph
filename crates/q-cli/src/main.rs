//! `q` — the Quatricmorph command-line interface.
//!
//! Data plane: reads the **Metadata Plane**, and the **Artifact Plane** only
//! when a command asks for an exact value (ARCHITECTURE.md §2.1).
//!
//! Every subcommand does real work against a real checkpoint. There is no
//! command that prints a plausible message without having done anything — a
//! command either performs its operation or exits non-zero with the reason and,
//! where relevant, the requirement ID covering the gap.
//!
//! ```text
//! q inspect  <DIR>                       parse headers, resolve names, summarize
//! q layers   <DIR>                       model -> layer hierarchy
//! q tensors  <DIR> [--layer N] [--role R] filtered tensor listing
//! q value    <DIR> <ADDRESS> --index i,j exact scalar via byte-range read
//! q slice    <DIR> <ADDRESS> --rows a:b --columns c:d
//! q query    <DIR> <WEIGHTQL>            plan, and execute if executable
//! q stats    <DIR> <ADDRESS> --rows --columns   CPU reference block statistics
//!            [--persist [--catalog DB]]   write the row into tensor_statistics
//! q backends                             what each compute backend can do
//! ```

use clap::{Parser, Subcommand};
use q_catalog::{Catalog, ConfigMetadata, StatisticsRow, SubjectKind, TensorFilter};
use q_gpu::{Backend, CpuBackend};
use q_nsir::{Registry, ResolvedModel};
use q_safetensors::{ingest_local, IngestOutcome};
use q_source::error::{QError, Result};
use q_source::role::TensorRole;
use q_source::LocalFsSource;
use q_statistics::TensorStatistics;
use q_tensor_runtime::{BlockExtent, Lod, TileId};
use q_weightql::{QueryEngine, QueryOutcome};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The fidelity of everything `q inspect` prints.
///
/// Headers, the shard index, and `config.json` only. **No weight byte is read**
/// (`q_safetensors::ingest::tests::ingestion_reads_only_headers_not_payload`).
/// `.plan/DATA_ARCHITECTURE.md` §8 names that fidelity `metadata`.
const INSPECT_FIDELITY: &str = "metadata";

#[derive(Parser, Debug)]
#[command(
    name = "q",
    about = "Quatricmorph — browse and query SafeTensors checkpoints without loading them",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Emit JSON instead of a human-readable table.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse headers, resolve names, and summarize a checkpoint.
    Inspect { model_dir: PathBuf },
    /// List the layer hierarchy.
    Layers { model_dir: PathBuf },
    /// List tensors, optionally filtered.
    Tensors {
        model_dir: PathBuf,
        #[arg(long)]
        layer: Option<u32>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// Show only tensors whose semantic role is `unknown`.
        #[arg(long)]
        unresolved: bool,
    },
    /// Read one exact scalar.
    Value {
        model_dir: PathBuf,
        /// A canonical address, a raw name, or an alias such as `Q[10]`.
        address: String,
        /// Comma-separated index, e.g. `100,42`.
        #[arg(long)]
        index: String,
    },
    /// Read an exact 2-D window.
    Slice {
        model_dir: PathBuf,
        address: String,
        #[arg(long)]
        rows: String,
        #[arg(long)]
        columns: String,
    },
    /// Plan a WeightQL query, executing it if a backend exists.
    Query {
        model_dir: PathBuf,
        /// e.g. `show tensor("Q[10][100,42]")`
        weightql: String,
    },
    /// CPU-reference statistics over one block.
    Stats {
        model_dir: PathBuf,
        address: String,
        #[arg(long, default_value = "0:64")]
        rows: String,
        #[arg(long, default_value = "0:64")]
        columns: String,
        #[arg(long, default_value_t = 16)]
        bins: usize,
        /// Write the result into `tensor_statistics` so it survives this process.
        ///
        /// The subject is the **whole tensor** only when the requested window
        /// covers it; anything smaller is persisted as a *block*, under its own
        /// `BlockId`. Labelling a 4×4 window as the tensor's statistics would be
        /// a false claim about 6 144 weights.
        #[arg(long)]
        persist: bool,
        /// Where to persist. Defaults to `<MODEL_DIR>/.quatricmorph/catalog.db`.
        ///
        /// Point `q-daemon --catalog` at the same file to serve the row over
        /// `GET /v1/tensors/{id}/statistics`.
        #[arg(long, value_name = "DB")]
        catalog: Option<PathBuf>,
    },
    /// Report what each compute backend can actually do.
    Backends,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            if let Some(req) = e.requirement_id() {
                eprintln!("       this gap is tracked as requirement {req} in STATUS.md");
            }
            ExitCode::FAILURE
        }
    }
}

/// Open a checkpoint: ingest metadata, resolve names, catalog them.
///
/// Headers only. Opening a 600 GB checkpoint here reads a few megabytes.
fn open(model_dir: &Path) -> Result<(LocalFsSource, Catalog, String)> {
    open_with_catalog(model_dir, None)
}

/// Where `--persist` writes when no `--catalog` is given.
///
/// Beside the checkpoint, in a directory of ours — never over an artifact.
/// `.plan/DATA_ARCHITECTURE.md` §2: *"Conversion outputs go beside the
/// checkpoint or into a cache directory, never over it."*
fn default_catalog_path(model_dir: &Path) -> PathBuf {
    model_dir.join(".quatricmorph").join("catalog.db")
}

/// As [`open`], but persisting the catalog to `catalog_path` when one is given.
///
/// The model and its tensor rows are upserted either way, so a file catalog is
/// self-contained: a daemon started against it can resolve the tensor a
/// statistics row belongs to.
fn open_with_catalog(
    model_dir: &Path,
    catalog_path: Option<&Path>,
) -> Result<(LocalFsSource, Catalog, String)> {
    let ingested = ingest_local(model_dir)?;
    let registry = Registry::builtin()?;
    let resolved = ResolvedModel::build(
        &registry,
        ingested.manifest.model_type().as_deref(),
        ingested.manifest.declared_architecture().as_deref(),
        ingested.descriptors.clone(),
    )?;
    let catalog = match catalog_path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| QError::Io {
                    path: parent.to_path_buf(),
                    source: e,
                })?;
            }
            Catalog::open(path)?
        }
        None => Catalog::open_in_memory()?,
    };
    catalog.upsert_resolved(
        ingested.model_id,
        &ingested.manifest.root_uri,
        &ingested.manifest.source_key,
        &ingested.manifest.revision,
        &ingested.manifest.fingerprint(),
        &resolved.resolver_id,
        &ConfigMetadata::from_config(ingested.manifest.config.as_ref()),
        &resolved,
    )?;
    Ok((
        LocalFsSource::open(model_dir)?,
        catalog,
        ingested.model_id.to_hex(),
    ))
}

/// What `q inspect` reports.
///
/// Two kinds of number live here and the distinction is load-bearing:
///
/// * **Observed** — `tensors`, `parameters`, `described_payload_bytes`,
///   `shard_count`, `bytes_read`, and `layer_count` whenever the descriptors
///   show one. Summed from the shard headers, so they are facts about the
///   checkpoint.
/// * **Declared** — `hidden_size`, `intermediate_size`, `num_attention_heads`,
///   `num_key_value_heads`, `vocab_size`, `torch_dtype`. Copied out of
///   `config.json`. Absent when the checkpoint has no config, or when a field
///   is unusable, and then they render as `null` — **never `0`**.
#[derive(Debug, Serialize)]
struct InspectReport {
    model_id: String,
    source_key: String,
    model_type: Option<String>,
    resolver: String,
    shard_count: usize,
    tensors: usize,
    parameters: u64,
    described_payload_bytes: u64,
    bytes_read: u64,
    unresolved_tensors: usize,
    hidden_size: Option<u32>,
    layer_count: Option<u32>,
    intermediate_size: Option<u32>,
    num_attention_heads: Option<u32>,
    num_key_value_heads: Option<u32>,
    vocab_size: Option<u32>,
    torch_dtype: Option<String>,
    fidelity: String,
}

impl InspectReport {
    fn build(ingested: &IngestOutcome, resolved: &ResolvedModel) -> Self {
        let config = ConfigMetadata::from_config(ingested.manifest.config.as_ref());
        let observed = q_catalog::observed_layer_count(&resolved.descriptors);
        Self {
            model_id: ingested.model_id.to_hex(),
            source_key: ingested.manifest.source_key.clone(),
            model_type: ingested.manifest.model_type(),
            resolver: resolved.resolver_id.clone(),
            shard_count: ingested.shard_count(),
            tensors: ingested.tensor_count(),
            parameters: ingested.total_parameters(),
            described_payload_bytes: ingested.described_payload_bytes,
            bytes_read: ingested.bytes_read,
            unresolved_tensors: resolved.unresolved_count(),
            layer_count: config.layer_count(observed),
            hidden_size: config.hidden_size,
            intermediate_size: config.intermediate_size,
            num_attention_heads: config.num_attention_heads,
            num_key_value_heads: config.num_key_value_heads,
            vocab_size: config.vocab_size,
            torch_dtype: config.torch_dtype,
            fidelity: INSPECT_FIDELITY.to_string(),
        }
    }

    fn render_text(&self) -> String {
        /// An absent declared field prints `null`, never `0`.
        fn opt<T: std::fmt::Display>(v: &Option<T>) -> String {
            v.as_ref()
                .map(T::to_string)
                .unwrap_or_else(|| "null".into())
        }
        fn line(out: &mut String, key: &str, value: String) {
            out.push_str(&format!("{key:<20}{value}\n"));
        }
        let mut s = String::new();
        line(&mut s, "model_id", self.model_id.clone());
        line(&mut s, "source", self.source_key.clone());
        line(&mut s, "model_type", opt(&self.model_type));
        line(&mut s, "resolver", self.resolver.clone());
        line(&mut s, "shards", self.shard_count.to_string());
        line(&mut s, "tensors", self.tensors.to_string());
        line(&mut s, "parameters", self.parameters.to_string());
        line(
            &mut s,
            "payload described",
            format!("{} bytes", self.described_payload_bytes),
        );
        line(
            &mut s,
            "bytes actually read",
            format!(
                "{} bytes (headers + index only — no weights loaded)",
                self.bytes_read
            ),
        );
        line(
            &mut s,
            "unresolved tensors",
            format!(
                "{}{}",
                self.unresolved_tensors,
                if self.unresolved_tensors > 0 {
                    "  (role `unknown` — the resolver did not guess)"
                } else {
                    ""
                }
            ),
        );
        s.push_str("-- declared by config.json (null = not declared, never 0) --\n");
        line(&mut s, "hidden_size", opt(&self.hidden_size));
        line(&mut s, "layer_count", opt(&self.layer_count));
        line(&mut s, "intermediate_size", opt(&self.intermediate_size));
        line(&mut s, "attention_heads", opt(&self.num_attention_heads));
        line(&mut s, "key_value_heads", opt(&self.num_key_value_heads));
        line(&mut s, "vocab_size", opt(&self.vocab_size));
        line(&mut s, "torch_dtype", opt(&self.torch_dtype));
        line(&mut s, "fidelity", self.fidelity.clone());
        s
    }
}

/// What a `q stats` run actually measured.
///
/// The distinction is the honesty of the whole command. A window that covers the
/// tensor is the tensor's statistics; anything smaller is a **block**, and it is
/// stored under its own `BlockId` so nothing can later read it as a statement
/// about the whole tensor. `BlockId` is `TileId::for_block` by
/// `docs/decisions/ADR-011-content-derived-identifiers.md`, so the catalog row
/// and the tile address are one identity rather than two.
#[derive(Debug, Clone)]
struct StatsSubject {
    id: String,
    kind: SubjectKind,
    /// `true` when the measured window is the entire tensor.
    covers_whole_tensor: bool,
}

impl StatsSubject {
    fn of(descriptor: &q_source::TensorDescriptor, extent: &BlockExtent) -> Self {
        let whole = descriptor.shape.len() == 2
            && extent.row_start == 0
            && extent.column_start == 0
            && extent.row_end >= descriptor.shape[0]
            && extent.column_end >= descriptor.shape[1];
        if whole {
            Self {
                id: descriptor.tensor_id.to_hex(),
                kind: SubjectKind::Tensor,
                covers_whole_tensor: true,
            }
        } else {
            Self {
                id: TileId::for_block(descriptor.tensor_id, Lod::Block, extent).to_hex(),
                kind: SubjectKind::Block,
                covers_whole_tensor: false,
            }
        }
    }
}

/// Persist one statistics row, returning its `statistics_id`.
fn persist_statistics(
    catalog: &Catalog,
    subject: &StatsSubject,
    statistics: TensorStatistics,
) -> Result<String> {
    let row = StatisticsRow::new(&subject.id, subject.kind, statistics)?;
    catalog.put_statistics(&row)?;
    Ok(row.statistics_id)
}

/// `q stats --json`. Wraps the statistics so the JSON carries what the human
/// form does: the fidelity label, the subject, and where the row went.
#[derive(Debug, Serialize)]
struct StatsReport<'a> {
    subject_id: &'a str,
    subject_kind: &'a str,
    covers_whole_tensor: bool,
    #[serde(flatten)]
    statistics: &'a TensorStatistics,
    /// `"aggregate"` or `"sampled"`. Derived from `approximate`, never spelled.
    fidelity: &'static str,
    /// `null` unless `--persist` was given.
    statistics_id: Option<&'a str>,
}

impl<'a> StatsReport<'a> {
    fn build(
        subject: &'a StatsSubject,
        statistics: &'a TensorStatistics,
        statistics_id: Option<&'a str>,
    ) -> Self {
        Self {
            subject_id: &subject.id,
            subject_kind: subject.kind.as_str(),
            covers_whole_tensor: subject.covers_whole_tensor,
            statistics,
            fidelity: statistics.fidelity().as_str(),
            statistics_id,
        }
    }
}

/// Pretty-print JSON, mapping the serializer error into the shared error type.
fn json_out<T: serde::Serialize>(value: &T) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|e| QError::json("cli json output", e))
}

fn parse_index(s: &str) -> Result<Vec<u64>> {
    s.split(',')
        .map(|p| {
            p.trim()
                .parse::<u64>()
                .map_err(|_| QError::QueryRejected(format!("`{p}` is not an index component")))
        })
        .collect()
}

fn parse_range(s: &str) -> Result<(u64, u64)> {
    let (a, b) = s
        .split_once(':')
        .ok_or_else(|| QError::QueryRejected(format!("`{s}` is not a range like `0:256`")))?;
    Ok((
        a.trim()
            .parse()
            .map_err(|_| QError::QueryRejected(format!("`{a}` is not an integer")))?,
        b.trim()
            .parse()
            .map_err(|_| QError::QueryRejected(format!("`{b}` is not an integer")))?,
    ))
}

fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Inspect { model_dir } => {
            let ingested = ingest_local(model_dir)?;
            let registry = Registry::builtin()?;
            let resolved = ResolvedModel::build(
                &registry,
                ingested.manifest.model_type().as_deref(),
                ingested.manifest.declared_architecture().as_deref(),
                ingested.descriptors.clone(),
            )?;
            let report = InspectReport::build(&ingested, &resolved);
            if cli.json {
                println!("{}", json_out(&report)?);
            } else {
                print!("{}", report.render_text());
            }
        }

        Command::Layers { model_dir } => {
            let (_, catalog, model_id) = open(model_dir)?;
            let layers = catalog.list_layers(&model_id)?;
            if cli.json {
                println!("{}", json_out(&layers)?);
            } else {
                println!(
                    "{:>5}  {:>8}  {:>14}  {:>14}",
                    "layer", "tensors", "parameters", "bytes"
                );
                for l in layers {
                    println!(
                        "{:>5}  {:>8}  {:>14}  {:>14}",
                        l.layer_index, l.tensor_count, l.parameter_count, l.payload_bytes
                    );
                }
            }
        }

        Command::Tensors {
            model_dir,
            layer,
            role,
            limit,
            unresolved,
        } => {
            let (_, catalog, model_id) = open(model_dir)?;
            let rows = if *unresolved {
                catalog.find_by_role(&model_id, TensorRole::Unknown, *layer)?
            } else {
                catalog.list_tensors(
                    &model_id,
                    &TensorFilter {
                        layer_index: *layer,
                        role: role.as_deref().map(TensorRole::parse),
                        limit: Some(*limit),
                        ..Default::default()
                    },
                )?
            };
            if cli.json {
                println!("{}", json_out(&rows)?);
            } else {
                for r in rows {
                    println!(
                        "{:<64} {:>12?} {:<6} {}",
                        r.canonical_name, r.shape, r.dtype, r.role
                    );
                }
            }
        }

        Command::Value {
            model_dir,
            address,
            index,
        } => {
            let (source, catalog, model_id) = open(model_dir)?;
            let engine = QueryEngine::with_source(&catalog, &model_id, &source)?;
            let idx = parse_index(index)?;
            let list = idx.iter().map(u64::to_string).collect::<Vec<_>>().join(",");
            match engine.run(&format!(
                r#"SELECT value FROM tensor("{address}") AT [{list}]"#
            ))? {
                QueryOutcome::Scalar { read, .. } => {
                    if cli.json {
                        println!("{}", json_out(&read)?);
                    } else {
                        println!("{}", read.value);
                        eprintln!(
                            "  {} at [{list}] — {} bytes read from {} at offset {} ({})",
                            read.canonical_name,
                            read.bytes_read,
                            read.shard_uri,
                            read.byte_offset,
                            read.fidelity.as_str()
                        );
                    }
                }
                other => {
                    return Err(QError::QueryRejected(format!(
                        "expected a scalar, got {other:?}"
                    )))
                }
            }
        }

        Command::Slice {
            model_dir,
            address,
            rows,
            columns,
        } => {
            let (source, catalog, model_id) = open(model_dir)?;
            let engine = QueryEngine::with_source(&catalog, &model_id, &source)?;
            let (r0, r1) = parse_range(rows)?;
            let (c0, c1) = parse_range(columns)?;
            match engine.run(&format!(
                r#"SELECT slice FROM tensor("{address}") ROWS {r0}:{r1} COLUMNS {c0}:{c1}"#
            ))? {
                QueryOutcome::Slice { read, .. } => {
                    if cli.json {
                        println!("{}", json_out(&read)?);
                    } else {
                        for i in 0..read.rows() {
                            let line: Vec<String> = (0..read.columns())
                                .map(|j| format!("{:>12.6}", read.get(i, j).unwrap_or(f64::NAN)))
                                .collect();
                            println!("{}", line.join(" "));
                        }
                        eprintln!(
                            "  {} [{r0}:{r1}, {c0}:{c1}] — {} bytes read ({})",
                            read.canonical_name,
                            read.bytes_read,
                            read.fidelity.as_str()
                        );
                    }
                }
                other => {
                    return Err(QError::QueryRejected(format!(
                        "expected a slice, got {other:?}"
                    )))
                }
            }
        }

        Command::Query {
            model_dir,
            weightql,
        } => {
            let (source, catalog, model_id) = open(model_dir)?;
            let engine = QueryEngine::with_source(&catalog, &model_id, &source)?;
            let outcome = engine.run(weightql)?;
            if cli.json {
                println!("{}", json_out(&outcome)?);
            } else {
                let plan = outcome.plan();
                println!("plan       {}", plan.plan_id);
                println!("expression {}", plan.expression);
                println!("shape      {:?}", plan.output_shape);
                println!("est. read  {} bytes", plan.estimated_read_bytes);
                match &outcome {
                    QueryOutcome::Scalar { read, .. } => {
                        println!("result     {} ({})", read.value, read.fidelity.as_str());
                    }
                    QueryOutcome::Slice { read, .. } => {
                        println!(
                            "result     {}x{} values ({})",
                            read.rows(),
                            read.columns(),
                            read.fidelity.as_str()
                        );
                    }
                    QueryOutcome::Planned(p) => {
                        println!("result     NOT EXECUTED");
                        if let Some(req) = &p.blocked_by {
                            println!("blocked by {req}");
                        }
                        if let Some(reason) = &p.blocked_reason {
                            println!("           {reason}");
                        }
                    }
                }
            }
        }

        Command::Stats {
            model_dir,
            address,
            rows,
            columns,
            bins,
            persist,
            catalog: catalog_path,
        } => {
            let resolved_catalog_path = if *persist {
                Some(
                    catalog_path
                        .clone()
                        .unwrap_or_else(|| default_catalog_path(model_dir)),
                )
            } else {
                catalog_path.clone()
            };
            let (source, catalog, model_id) =
                open_with_catalog(model_dir, resolved_catalog_path.as_deref())?;
            let row = catalog
                .get_by_canonical_name(&model_id, address)?
                .ok_or_else(|| QError::NotFound(format!("tensor `{address}`")))?;
            let descriptor = row.to_descriptor()?;
            let (r0, r1) = parse_range(rows)?;
            let (c0, c1) = parse_range(columns)?;
            // `clamped_to` refuses any rank but 2 — a rank-3 or rank-4 tensor is
            // not silently flattened into a matrix
            // (`docs/decisions/ADR-010-tensor-rank-ceiling.md`).
            let extent = BlockExtent::new(r0, r1, c0, c1)?.clamped_to(&descriptor.shape)?;
            let stats = CpuBackend.block_statistics(&source, &descriptor, extent, *bins)?;
            let subject = StatsSubject::of(&descriptor, &extent);
            let persisted = if *persist {
                Some(persist_statistics(&catalog, &subject, stats.clone())?)
            } else {
                None
            };
            if cli.json {
                println!(
                    "{}",
                    json_out(&StatsReport::build(&subject, &stats, persisted.as_deref()))?
                );
            } else {
                println!("tensor    {}", descriptor.canonical_name);
                println!(
                    "block     [{r0}:{r1}, {c0}:{c1}]  ({} elements)",
                    stats.count
                );
                println!("min/max   {} / {}", stats.min_value, stats.max_value);
                println!("mean      {}", stats.mean);
                println!("stddev    {}", stats.std_dev());
                println!("L1 / L2   {} / {}", stats.l1_norm, stats.l2_norm);
                println!(
                    "ratios    zero {:.4}  positive {:.4}  negative {:.4}",
                    stats.zero_ratio, stats.positive_ratio, stats.negative_ratio
                );
                println!(
                    "backend   {} (algorithm v{}, {})",
                    stats.backend,
                    stats.algorithm_version,
                    if stats.approximate {
                        "APPROXIMATE"
                    } else {
                        "exact"
                    }
                );
                println!(
                    "fidelity  {}  ({})",
                    stats.fidelity().as_str(),
                    if stats.approximate {
                        "computed over a SAMPLE — not every element of the block was read"
                    } else {
                        "every element of the block was read; exact for this block"
                    }
                );
                println!("subject   {} {}", subject.kind.as_str(), subject.id);
                if let Some(statistics_id) = &persisted {
                    println!(
                        "persisted {statistics_id} -> {}",
                        resolved_catalog_path
                            .as_deref()
                            .unwrap_or(Path::new("<in-memory>"))
                            .display()
                    );
                }
            }
        }

        Command::Backends => {
            let backends: Vec<q_gpu::ComputeCapabilities> = vec![
                CpuBackend.capabilities(),
                q_cuda::CudaBackend::new().capabilities(),
            ];
            if cli.json {
                println!("{}", json_out(&backends)?);
            } else {
                for c in backends {
                    println!("{} — {}", c.backend_id, c.display_name);
                    println!(
                        "  memory {} MiB   statistics {}   matmul {}   histogram {}",
                        c.device_memory_bytes / (1024 * 1024),
                        c.supports_statistics,
                        c.supports_matmul,
                        c.supports_histogram
                    );
                    if !c.hardware_verified {
                        println!(
                            "  ⚠ HARDWARE-UNVERIFIED — never executed on the target device{}",
                            c.caveat_requirement
                                .map(|r| format!(" (requirement {r})"))
                                .unwrap_or_default()
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .canonicalize()
            .expect("run fixtures/generate_fixtures.py")
    }

    fn report(dir: &Path) -> Result<InspectReport> {
        report_from(ingest_local(dir)?)
    }

    fn report_from(ingested: q_safetensors::IngestOutcome) -> Result<InspectReport> {
        let registry = Registry::builtin()?;
        let resolved = ResolvedModel::build(
            &registry,
            ingested.manifest.model_type().as_deref(),
            ingested.manifest.declared_architecture().as_deref(),
            ingested.descriptors.clone(),
        )?;
        Ok(InspectReport::build(&ingested, &resolved))
    }

    #[test]
    fn inspect_reports_the_fixture_config_metadata_at_metadata_fidelity() {
        let r = report(&fixture("tiny-llama-2shard")).unwrap();
        // fixtures/tiny-llama-2shard/config.json
        assert_eq!(r.hidden_size, Some(48));
        assert_eq!(r.layer_count, Some(12));
        assert_eq!(r.intermediate_size, Some(64));
        assert_eq!(r.num_attention_heads, Some(8));
        assert_eq!(r.num_key_value_heads, Some(2));
        assert_eq!(r.vocab_size, Some(64));
        assert_eq!(r.torch_dtype.as_deref(), Some("float32"));
        // From the manifest, not from config arithmetic.
        assert_eq!(r.parameters, 302_256);
        assert_eq!(r.described_payload_bytes, 1_196_736);
        assert_eq!(r.tensors, 111);
        assert_eq!(r.shard_count, 2);
        assert_eq!(r.fidelity, "metadata");
        // Nothing but headers and the index was read.
        assert!(r.bytes_read < r.described_payload_bytes / 10);
    }

    #[test]
    fn inspect_renders_absent_config_fields_as_null_never_zero() {
        // A checkpoint with no `config.json` at all. That such a checkpoint
        // ingests is proved in `q_safetensors::ingest::tests::
        // a_checkpoint_without_a_config_json_still_ingests`; what is under test
        // here is only how the absence renders.
        let mut ingested = ingest_local(fixture("tiny-llama-single")).unwrap();
        ingested.manifest.config = None;
        let r = report_from(ingested).unwrap();
        let wire = serde_json::to_value(&r).unwrap();
        for key in [
            "hidden_size",
            "intermediate_size",
            "num_attention_heads",
            "num_key_value_heads",
            "vocab_size",
            "torch_dtype",
            "model_type",
        ] {
            assert!(
                wire[key].is_null(),
                "{key} was {}, expected null",
                wire[key]
            );
            assert_ne!(wire[key], serde_json::json!(0), "{key} rendered as zero");
        }
        // `layer_count` is *not* in that list, and the difference matters: a
        // `layers.N.` segment in a tensor name is structural, so even the
        // generic resolver — which claims no semantics here, `model_type` being
        // absent — still observes the layer index. Observed beats declared, so
        // the count survives the config's disappearance.
        assert_eq!(r.layer_count, Some(1));
        assert_eq!(r.resolver, "generic");
        // The counts that come from the manifest are still exact.
        assert_eq!(r.tensors, 10);
        assert_eq!(r.shard_count, 1);
        assert_eq!(r.fidelity, "metadata");
        // An absent declared field prints as `null` in the human form too.
        assert!(r.render_text().contains("hidden_size         null"));
    }

    // --- q stats --persist ---------------------------------------------------

    const Q10: &str = "model.layers[10].self_attention.query_projection.weight";

    /// Run the `--persist` path against a temporary catalog, exactly as the
    /// command does, and hand back what it wrote.
    fn stats_and_persist(
        db: &Path,
        address: &str,
        rows: (u64, u64),
        columns: (u64, u64),
        bins: usize,
    ) -> Result<(String, StatsSubject, TensorStatistics)> {
        let model_dir = fixture("tiny-llama-2shard");
        let (source, catalog, model_id) = open_with_catalog(&model_dir, Some(db))?;
        let row = catalog
            .get_by_canonical_name(&model_id, address)?
            .ok_or_else(|| QError::NotFound(format!("tensor `{address}`")))?;
        let descriptor = row.to_descriptor()?;
        let extent = BlockExtent::new(rows.0, rows.1, columns.0, columns.1)?
            .clamped_to(&descriptor.shape)?;
        let stats = CpuBackend.block_statistics(&source, &descriptor, extent, bins)?;
        let subject = StatsSubject::of(&descriptor, &extent);
        let id = persist_statistics(&catalog, &subject, stats.clone())?;
        Ok((id, subject, stats))
    }

    #[test]
    fn a_persisted_statistic_is_readable_after_the_catalog_is_closed_and_reopened() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let (statistics_id, subject, written) =
            stats_and_persist(&db, Q10, (100, 104), (40, 44), 16).unwrap();

        // Fresh handle on the same file: nothing of the first one survives.
        let catalog = Catalog::open(&db).unwrap();
        let back = catalog.get_statistics(&subject.id, 1).unwrap().unwrap();
        assert_eq!(back.statistics_id, statistics_id);
        assert_eq!(back.statistics, written);
        // The block's 16 values, from `--rows 100:104 --columns 40:44`.
        assert_eq!(back.statistics.count, 16);
        assert_eq!(back.statistics.histogram.bins(), 16);
        assert_eq!(back.statistics.histogram.total(), 16);
        assert_eq!(back.statistics.backend, "cpu-reference");
        assert!(!back.statistics.approximate);
        assert_eq!(back.fidelity().as_str(), "aggregate");
    }

    #[test]
    fn a_persisted_block_statistic_matches_the_hand_computed_golden_values() {
        // `scripts/baseline.json`'s `cli_golden.stats_*` for this exact window,
        // derived from `fixtures/tiny-llama-2shard/golden.json`'s f32 bit
        // patterns rather than from this code (see `.plan/evidence/QM-0001.md`).
        // The round trip must not perturb them.
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let (_, subject, _) = stats_and_persist(&db, Q10, (100, 104), (40, 44), 16).unwrap();
        let s = Catalog::open(&db)
            .unwrap()
            .get_statistics(&subject.id, 1)
            .unwrap()
            .unwrap()
            .statistics;
        assert_eq!(s.min_value, -0.04204120859503746);
        assert_eq!(s.max_value, 0.0300398301333189);
        assert_eq!(s.mean, -0.005001873796572909);
        assert_eq!(s.l1_norm, 0.27181090926751494);
        assert_eq!(s.l2_norm, 0.08061549393964319);
    }

    #[test]
    fn a_partial_window_is_persisted_as_a_block_never_as_the_whole_tensor() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let (_, subject, _) = stats_and_persist(&db, Q10, (100, 104), (40, 44), 16).unwrap();
        assert_eq!(subject.kind, SubjectKind::Block);
        assert!(!subject.covers_whole_tensor);

        // The tensor's own id holds nothing: 16 of 6 144 weights is not the
        // tensor's statistics, and must never be readable as if it were.
        let catalog = Catalog::open(&db).unwrap();
        let model_dir = fixture("tiny-llama-2shard");
        let (_, mem, model_id) = open(&model_dir).unwrap();
        let tensor_id = mem
            .get_by_canonical_name(&model_id, Q10)
            .unwrap()
            .unwrap()
            .tensor_id;
        assert_ne!(subject.id, tensor_id);
        assert!(catalog.get_statistics(&tensor_id, 1).unwrap().is_none());
    }

    #[test]
    fn a_window_covering_the_whole_tensor_is_persisted_as_the_tensor() {
        // Q[10] is [128, 48] = 6 144 weights — the count the data contract shows.
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let (_, subject, stats) = stats_and_persist(&db, Q10, (0, 128), (0, 48), 64).unwrap();
        assert_eq!(subject.kind, SubjectKind::Tensor);
        assert!(subject.covers_whole_tensor);
        assert_eq!(stats.count, 6144);
        assert_eq!(stats.histogram.bins(), 64);

        let model_dir = fixture("tiny-llama-2shard");
        let (_, mem, model_id) = open(&model_dir).unwrap();
        let tensor_id = mem
            .get_by_canonical_name(&model_id, Q10)
            .unwrap()
            .unwrap()
            .tensor_id;
        assert_eq!(subject.id, tensor_id);
        let back = Catalog::open(&db)
            .unwrap()
            .get_statistics(&tensor_id, 1)
            .unwrap()
            .unwrap();
        assert_eq!(back.subject_kind, SubjectKind::Tensor);
        assert_eq!(back.statistics.count, 6144);
        assert_eq!(back.statistics.histogram.total(), 6144);
    }

    #[test]
    fn persisting_statistics_for_a_rank_one_tensor_is_refused_not_flattened() {
        // `input_layernorm.weight` is rank 1. A block extent applies to rank-2
        // tensors; reinterpreting a rank-1 (or rank-4) tensor as a matrix would
        // produce a confidently wrong picture, which
        // `docs/decisions/ADR-010-tensor-rank-ceiling.md` refuses by design.
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let err = stats_and_persist(
            &db,
            "model.layers[10].normalization.input_normalization.weight",
            (0, 8),
            (0, 8),
            8,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("rank-2 tensors"),
            "expected a rank refusal, got: {err}"
        );
        assert!(err.to_string().contains("got rank 1"), "{err}");
        // Nothing was written on the way to refusing.
        assert!(Catalog::open(&db)
            .unwrap()
            .list_statistics(&"0".repeat(32))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn an_out_of_bounds_window_is_refused_before_any_row_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        // Q[10] has 128 rows; row 500 is not in it.
        let err = stats_and_persist(&db, Q10, (500, 504), (0, 4), 8).unwrap_err();
        assert!(
            matches!(err, QError::IndexOutOfBounds { .. }),
            "expected an out-of-bounds refusal, got: {err}"
        );
    }

    #[test]
    fn the_default_persist_path_sits_beside_the_checkpoint_not_over_it() {
        let dir = fixture("tiny-llama-2shard");
        let path = default_catalog_path(&dir);
        assert_eq!(path.parent().unwrap(), dir.join(".quatricmorph"));
        assert_eq!(path.file_name().unwrap(), "catalog.db");
        // It is not any artifact the checkpoint owns.
        for artifact in [
            "config.json",
            "model.safetensors.index.json",
            "model-00001-of-00002.safetensors",
        ] {
            assert_ne!(path, dir.join(artifact));
        }
    }

    #[test]
    fn the_stats_json_carries_a_fidelity_label_and_the_subject_it_measured() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let (statistics_id, subject, stats) =
            stats_and_persist(&db, Q10, (100, 104), (40, 44), 16).unwrap();
        let wire = serde_json::to_value(StatsReport::build(&subject, &stats, Some(&statistics_id)))
            .unwrap();
        assert_eq!(wire["fidelity"], serde_json::json!("aggregate"));
        assert_eq!(wire["approximate"], serde_json::json!(false));
        assert_eq!(wire["subject_kind"], serde_json::json!("block"));
        assert_eq!(wire["covers_whole_tensor"], serde_json::json!(false));
        assert_eq!(wire["count"], serde_json::json!(16));
        assert_eq!(wire["statistics_id"], serde_json::json!(statistics_id));

        // Without `--persist` there is no row, and the field is `null` rather
        // than an empty string or a fabricated id.
        let unpersisted = serde_json::to_value(StatsReport::build(&subject, &stats, None)).unwrap();
        assert!(unpersisted["statistics_id"].is_null(), "{unpersisted}");
        assert_eq!(unpersisted["fidelity"], serde_json::json!("aggregate"));
    }

    #[test]
    fn a_sampled_statistic_reaches_the_report_labelled_sampled() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("catalog.db");
        let (_, subject, mut stats) =
            stats_and_persist(&db, Q10, (100, 104), (40, 44), 16).unwrap();
        stats.approximate = true;
        let report = StatsReport::build(&subject, &stats, None);
        assert_eq!(report.fidelity, "sampled");
        let wire = serde_json::to_value(&report).unwrap();
        assert_eq!(wire["fidelity"], serde_json::json!("sampled"));
        assert_eq!(wire["approximate"], serde_json::json!(true));
    }

    #[test]
    fn inspect_prints_the_config_metadata_in_the_human_readable_form() {
        let r = report(&fixture("tiny-llama-2shard")).unwrap();
        let text = r.render_text();
        assert!(text.contains("hidden_size         48"), "{text}");
        assert!(text.contains("layer_count         12"), "{text}");
        assert!(text.contains("parameters          302256"), "{text}");
        assert!(text.contains("fidelity            metadata"), "{text}");
        // An absent field prints as `null`, never as `0`.
        let empty = InspectReport {
            hidden_size: None,
            ..r
        };
        assert!(empty.render_text().contains("hidden_size         null"));
    }
}
