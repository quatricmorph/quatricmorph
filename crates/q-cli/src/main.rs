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
//! q backends                             what each compute backend can do
//! ```

use clap::{Parser, Subcommand};
use q_catalog::{Catalog, ConfigMetadata, TensorFilter};
use q_gpu::{Backend, CpuBackend};
use q_nsir::{Registry, ResolvedModel};
use q_safetensors::{ingest_local, IngestOutcome};
use q_source::error::{QError, Result};
use q_source::role::TensorRole;
use q_source::LocalFsSource;
use q_tensor_runtime::BlockExtent;
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
    let ingested = ingest_local(model_dir)?;
    let registry = Registry::builtin()?;
    let resolved = ResolvedModel::build(
        &registry,
        ingested.manifest.model_type().as_deref(),
        ingested.manifest.declared_architecture().as_deref(),
        ingested.descriptors.clone(),
    )?;
    let catalog = Catalog::open_in_memory()?;
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
        } => {
            let (source, catalog, model_id) = open(model_dir)?;
            let row = catalog
                .get_by_canonical_name(&model_id, address)?
                .ok_or_else(|| QError::NotFound(format!("tensor `{address}`")))?;
            let descriptor = row.to_descriptor()?;
            let (r0, r1) = parse_range(rows)?;
            let (c0, c1) = parse_range(columns)?;
            let extent = BlockExtent::new(r0, r1, c0, c1)?.clamped_to(&descriptor.shape)?;
            let stats = CpuBackend.block_statistics(&source, &descriptor, extent, *bins)?;
            if cli.json {
                println!("{}", json_out(&stats)?);
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
