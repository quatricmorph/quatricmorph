//! # q-architecture — Metadata Plane
//!
//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §4.2).
//!
//! The architecture-plugin registry. A plugin is a declarative manifest
//! (`architectures/<id>/plugin.toml`) describing:
//!
//! * which models it claims (by `config.json` `model_type` / `architectures`);
//! * a rule table mapping tensor-name patterns to NSIR roles, operations, and
//!   axis labels;
//! * the contextual aliases of ARCHITECTURE.md §6.2.
//!
//! `q-nsir` applies these manifests; this crate only loads and selects them.
//!
//! ## Never guess
//!
//! ARCHITECTURE.md §4.2: *"The resolver must be allowed to return `unknown`. It
//! must never guess a semantic role just because two tensors share the same
//! shape."* Nothing in this crate inspects a shape. Selection is by declared
//! model type; resolution is by declared name pattern. A plugin marked
//! `implemented = false` never claims a model — the registry falls back to
//! `generic`, which returns `unknown` for names it does not know.
//!
//! ## Adding an architecture
//!
//! Copy `architectures/llama/plugin.toml`, change `[plugin].id`, `[match]`, and
//! the rule table, set `implemented = true`. No Rust change is needed unless the
//! family needs a structural concept the schema cannot express. `architectures/qwen`
//! (`QM-0010`, `NSIR-006`) is the worked example: it covers Qwen2 and Qwen3,
//! dense and MoE, including per-head query/key norms and `experts.N.*`
//! addressing, and required **no** change to this crate or to `q-nsir`.

use q_source::error::{QError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// How a rule's `name` is matched against a tensor name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    /// The whole tensor name equals `name`.
    Exact,
    /// The name, after any `…layers.N.` prefix is stripped, equals `name`.
    Suffix,
    /// Like `Suffix`, but only inside an `…experts.M.` segment.
    ExpertSuffix,
}

/// One name-to-semantics rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub match_kind: MatchKind,
    /// `q_source::TensorRole` spelling, e.g. `"attention_query_projection"`.
    pub role: String,
    #[serde(default)]
    pub component: Option<String>,
    pub operation: String,
    pub parameter: String,
    #[serde(default)]
    pub axes: Vec<String>,
}

/// A contextual alias (`Q`, `Att`, `MLP.down`, …).
///
/// `roles.len() > 1` means the alias is ambiguous *by design*; resolution
/// returns candidates instead of choosing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AliasRule {
    pub alias: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginIdentity {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub implemented: bool,
    #[serde(default = "default_nsir_version")]
    pub nsir_version: u32,
    #[serde(default)]
    pub notes: Option<String>,
}

fn default_nsir_version() -> u32 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MatchSpec {
    #[serde(default)]
    pub model_types: Vec<String>,
    #[serde(default)]
    pub architectures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamingSpec {
    #[serde(default = "default_stack")]
    pub stack: String,
    /// Path segment introducing the repeated layer stack, e.g. `layers`.
    #[serde(default = "default_layer_segment")]
    pub layer_segment: String,
    /// Path segment introducing MoE experts, e.g. `experts`.
    #[serde(default)]
    pub expert_segment: Option<String>,
}

fn default_stack() -> String {
    "language".to_string()
}

fn default_layer_segment() -> String {
    "layers".to_string()
}

impl Default for NamingSpec {
    fn default() -> Self {
        Self {
            stack: default_stack(),
            layer_segment: default_layer_segment(),
            expert_segment: None,
        }
    }
}

/// A parsed `plugin.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitecturePlugin {
    pub plugin: PluginIdentity,
    #[serde(default, rename = "match")]
    pub match_spec: MatchSpec,
    #[serde(default)]
    pub naming: NamingSpec,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub aliases: Vec<AliasRule>,
}

impl ArchitecturePlugin {
    pub fn parse(source_name: &str, text: &str) -> Result<Self> {
        toml::from_str(text)
            .map_err(|e| QError::malformed(source_name, format!("invalid plugin manifest: {e}")))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| QError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::parse(&path.to_string_lossy(), &text)
    }

    pub fn id(&self) -> &str {
        &self.plugin.id
    }

    pub fn is_implemented(&self) -> bool {
        self.plugin.implemented
    }

    /// Whether this plugin claims a model. An unimplemented plugin claims
    /// nothing, no matter what `[match]` says.
    pub fn claims(&self, model_type: Option<&str>, architecture: Option<&str>) -> bool {
        if !self.is_implemented() {
            return false;
        }
        if let Some(mt) = model_type {
            if self.match_spec.model_types.iter().any(|m| m == mt) {
                return true;
            }
        }
        if let Some(a) = architecture {
            if self.match_spec.architectures.iter().any(|m| m == a) {
                return true;
            }
        }
        false
    }

    /// Aliases grouped by alias string, preserving declared role order.
    pub fn alias_map(&self) -> BTreeMap<String, Vec<String>> {
        self.aliases
            .iter()
            .map(|a| (a.alias.clone(), a.roles.clone()))
            .collect()
    }
}

/// Model-level metadata **declared** by a checkpoint's `config.json`.
///
/// ARCHITECTURE.md §4.2 makes `config.json` this crate's input: it already
/// decides which plugin claims a model from `model_type` / `architectures`.
/// This is the rest of that same file, typed — the dimensions a summary needs
/// (`ARCHITECTURE.md` §9.2: LOD 0 carries *"parameter count, bytes, global
/// distributions"*).
///
/// Three rules hold throughout, and the tests name each one:
///
/// * **Declared, not observed.** Every field is what the checkpoint's author
///   wrote down. The shard headers are the authority on what is actually
///   stored, and where the two can disagree the header wins — see
///   [`Self::layer_count`] and the note on [`Self::torch_dtype`].
/// * **Absent is `None`, never `0`.** A field that is missing, of the wrong
///   JSON type, negative, or too large to fit becomes `None`. Zero would be a
///   claim about a model we did not read.
/// * **One bad key costs only that key.** The remaining fields still load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfigMetadata {
    pub hidden_size: Option<u32>,
    pub num_hidden_layers: Option<u32>,
    pub intermediate_size: Option<u32>,
    pub num_attention_heads: Option<u32>,
    pub num_key_value_heads: Option<u32>,
    pub vocab_size: Option<u32>,
    /// `torch_dtype` verbatim, e.g. `"float32"`.
    ///
    /// **Never used to infer any tensor's storage dtype.** A checkpoint may
    /// store individual tensors at other widths — `fixtures/tiny-llama-2shard`
    /// declares `float32` and holds two BF16 tensors — so this is a fact about
    /// the checkpoint's origin and nothing else. Deriving a `DType` from it
    /// would be the shape-guessing that ARCHITECTURE.md §4.2 forbids, in
    /// another costume.
    pub torch_dtype: Option<String>,
}

impl ModelConfigMetadata {
    /// Read the fields out of an already-parsed `config.json`.
    ///
    /// `None` — no `config.json` in the checkpoint — declares nothing.
    pub fn from_config(config: Option<&serde_json::Value>) -> Self {
        let Some(v) = config else {
            return Self::default();
        };
        Self {
            hidden_size: u32_field(v, "hidden_size"),
            num_hidden_layers: u32_field(v, "num_hidden_layers"),
            intermediate_size: u32_field(v, "intermediate_size"),
            num_attention_heads: u32_field(v, "num_attention_heads"),
            num_key_value_heads: u32_field(v, "num_key_value_heads"),
            vocab_size: u32_field(v, "vocab_size"),
            torch_dtype: v
                .get("torch_dtype")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
        }
    }

    /// Parse `config.json` text.
    ///
    /// Malformed JSON is refused with `source_name` as context; a *well-formed*
    /// file with unusable values is not an error, because the individual fields
    /// degrade to `None` on their own.
    pub fn parse(source_name: &str, text: &str) -> Result<Self> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| QError::json(source_name, e))?;
        Ok(Self::from_config(Some(&value)))
    }

    /// Whether the config declared nothing this type records.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The layer count to report, given what the manifest actually showed.
    ///
    /// `observed` — the number of layers the shard headers produced, by way of
    /// resolution — is exact, so it always wins, even against a config that
    /// disagrees. `num_hidden_layers` fills in only when nothing was observed:
    /// that is the generic-fallback case, where no plugin claimed the model and
    /// so no descriptor carries a layer index. Reporting `NULL` there would
    /// discard a fact the checkpoint states about itself.
    pub fn layer_count(&self, observed: Option<u32>) -> Option<u32> {
        observed.or(self.num_hidden_layers)
    }
}

/// A `u32` config field, or `None`.
///
/// `as_u64` already rejects strings, floats, and negatives; `try_into` rejects
/// anything past `u32::MAX`. Nothing is coerced, clamped, or rounded — a value
/// this cannot represent exactly is absent rather than wrong.
fn u32_field(value: &serde_json::Value, key: &str) -> Option<u32> {
    value.get(key)?.as_u64()?.try_into().ok()
}

/// Built-in manifests, embedded at compile time.
///
/// Embedding means resolution works regardless of the process's working
/// directory (tests, the daemon, the CLI); [`Registry::load_dir`] still allows
/// overriding or extending from disk.
pub const BUILTIN_GENERIC: &str = include_str!("../../../architectures/generic/plugin.toml");
pub const BUILTIN_LLAMA: &str = include_str!("../../../architectures/llama/plugin.toml");
pub const BUILTIN_QWEN: &str = include_str!("../../../architectures/qwen/plugin.toml");
pub const BUILTIN_KIMI: &str = include_str!("../../../architectures/kimi/plugin.toml");
pub const BUILTIN_DEEPSEEK: &str = include_str!("../../../architectures/deepseek/plugin.toml");

/// The set of loaded plugins, with selection logic.
#[derive(Debug, Clone)]
pub struct Registry {
    plugins: Vec<ArchitecturePlugin>,
}

impl Registry {
    /// The built-in registry: `generic`, `llama`, and `qwen` implemented;
    /// `kimi` and `deepseek` declared-but-unimplemented, so neither ever claims
    /// a model (`unimplemented_plugins_are_declared_and_never_claim`).
    pub fn builtin() -> Result<Self> {
        let plugins = [
            ("generic", BUILTIN_GENERIC),
            ("llama", BUILTIN_LLAMA),
            ("qwen", BUILTIN_QWEN),
            ("kimi", BUILTIN_KIMI),
            ("deepseek", BUILTIN_DEEPSEEK),
        ]
        .into_iter()
        .map(|(name, text)| ArchitecturePlugin::parse(name, text))
        .collect::<Result<Vec<_>>>()?;
        Ok(Self { plugins })
    }

    /// Load every `*/plugin.toml` under `dir`.
    pub fn load_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let mut plugins = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| QError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| QError::Io {
                path: dir.to_path_buf(),
                source: e,
            })?;
            let manifest = entry.path().join("plugin.toml");
            if manifest.is_file() {
                plugins.push(ArchitecturePlugin::load(&manifest)?);
            }
        }
        plugins.sort_by(|a, b| a.plugin.id.cmp(&b.plugin.id));
        Ok(Self { plugins })
    }

    pub fn plugins(&self) -> &[ArchitecturePlugin] {
        &self.plugins
    }

    pub fn get(&self, id: &str) -> Option<&ArchitecturePlugin> {
        self.plugins.iter().find(|p| p.id() == id)
    }

    /// The always-present fallback.
    pub fn generic(&self) -> Result<&ArchitecturePlugin> {
        self.get("generic")
            .ok_or_else(|| QError::NotFound("generic architecture plugin".into()))
    }

    /// IDs of plugins that exist as manifests but have no rule table yet.
    pub fn declared_but_unimplemented(&self) -> Vec<&str> {
        self.plugins
            .iter()
            .filter(|p| !p.is_implemented())
            .map(|p| p.id())
            .collect()
    }

    /// Select the plugin for a model.
    ///
    /// Highest-priority claimant wins; `generic` is returned when nothing
    /// claims the model. Returning `generic` is a correct outcome, not a
    /// failure — it means "resolve what is structurally evident, mark the rest
    /// unknown".
    pub fn select(
        &self,
        model_type: Option<&str>,
        architecture: Option<&str>,
    ) -> Result<Selection<'_>> {
        let mut best: Option<&ArchitecturePlugin> = None;
        for p in &self.plugins {
            if p.claims(model_type, architecture) {
                match best {
                    Some(b) if b.plugin.priority >= p.plugin.priority => {}
                    _ => best = Some(p),
                }
            }
        }
        match best {
            Some(p) => Ok(Selection {
                plugin: p,
                matched: true,
            }),
            None => Ok(Selection {
                plugin: self.generic()?,
                matched: false,
            }),
        }
    }
}

/// The outcome of [`Registry::select`].
#[derive(Debug, Clone, Copy)]
pub struct Selection<'a> {
    pub plugin: &'a ArchitecturePlugin,
    /// `false` when no plugin claimed the model and `generic` was used.
    pub matched: bool,
}

impl Selection<'_> {
    pub fn id(&self) -> &str {
        self.plugin.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture's own `config.json`, read from disk rather than duplicated
    /// here, so the expectations below are checked against the real file.
    fn fixture_config_text(fixture: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(fixture)
            .join("config.json");
        std::fs::read_to_string(&path).expect("run fixtures/generate_fixtures.py")
    }

    #[test]
    fn config_metadata_parses_every_declared_field_of_the_fixture() {
        let c =
            ModelConfigMetadata::parse("config.json", &fixture_config_text("tiny-llama-2shard"))
                .unwrap();
        // Hand-read from fixtures/tiny-llama-2shard/config.json.
        assert_eq!(c.hidden_size, Some(48));
        assert_eq!(c.num_hidden_layers, Some(12));
        assert_eq!(c.intermediate_size, Some(64));
        assert_eq!(c.num_attention_heads, Some(8));
        assert_eq!(c.num_key_value_heads, Some(2));
        assert_eq!(c.vocab_size, Some(64));
        assert_eq!(c.torch_dtype.as_deref(), Some("float32"));
        assert!(!c.is_empty());
    }

    #[test]
    fn an_absent_config_declares_nothing_rather_than_zero() {
        let c = ModelConfigMetadata::from_config(None);
        assert_eq!(c, ModelConfigMetadata::default());
        assert!(c.is_empty());
        // `None`, never `Some(0)` — zero would be a lie about a model whose
        // config we never saw.
        assert_eq!(c.hidden_size, None);
        assert_eq!(c.num_hidden_layers, None);
        assert_eq!(c.vocab_size, None);
        assert_eq!(c.torch_dtype, None);
    }

    #[test]
    fn a_field_of_the_wrong_type_is_none_and_the_rest_still_load() {
        let c = ModelConfigMetadata::parse(
            "config.json",
            r#"{"hidden_size": "big", "num_hidden_layers": 12, "vocab_size": 64,
                "torch_dtype": 32}"#,
        )
        .unwrap();
        assert_eq!(c.hidden_size, None);
        assert_eq!(c.torch_dtype, None);
        assert_eq!(c.num_hidden_layers, Some(12));
        assert_eq!(c.vocab_size, Some(64));
    }

    #[test]
    fn a_negative_or_oversized_field_is_none_rather_than_truncated() {
        let c = ModelConfigMetadata::parse(
            "config.json",
            r#"{"hidden_size": -48, "vocab_size": 4294967296, "num_hidden_layers": 12}"#,
        )
        .unwrap();
        assert_eq!(c.hidden_size, None);
        assert_eq!(c.vocab_size, None);
        assert_eq!(c.num_hidden_layers, Some(12));
    }

    #[test]
    fn a_missing_field_is_none_and_is_never_inferred_from_the_others() {
        // `num_attention_heads * head_dim == 128`, which is *not* this model's
        // hidden_size (48). Nothing may reconstruct an absent field from the
        // ones that happen to be present.
        let c = ModelConfigMetadata::parse(
            "config.json",
            r#"{"num_attention_heads": 8, "head_dim": 16, "intermediate_size": 64}"#,
        )
        .unwrap();
        assert_eq!(c.hidden_size, None);
        assert_eq!(c.num_hidden_layers, None);
        assert_eq!(c.num_attention_heads, Some(8));
        assert_eq!(c.intermediate_size, Some(64));
    }

    #[test]
    fn malformed_config_json_is_refused_with_context() {
        let err = ModelConfigMetadata::parse("config.json", "{ not json").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("config.json"), "message lacked context: {msg}");
        assert!(matches!(err, QError::Json { .. }), "{err:?}");
    }

    #[test]
    fn a_config_that_is_not_a_json_object_declares_nothing_rather_than_failing() {
        let c = ModelConfigMetadata::parse("config.json", "[1, 2, 3]").unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn an_unknown_config_key_is_ignored_rather_than_failing_the_parse() {
        let c = ModelConfigMetadata::parse(
            "config.json",
            r#"{"quantization_config": {"bits": 4}, "hidden_size": 48}"#,
        )
        .unwrap();
        assert_eq!(c.hidden_size, Some(48));
    }

    #[test]
    fn config_torch_dtype_is_recorded_as_declared_not_used_to_infer_tensor_dtype() {
        // `tiny-llama-2shard` declares `torch_dtype: "float32"` and yet stores
        // two BF16 tensors (`ingest::tests::bf16_tensors_are_described_with_the
        // _right_width`). The declared value is a fact about the checkpoint's
        // origin, not an authority over any tensor's storage width — the shard
        // header is the only authority. So this type carries the string
        // verbatim and offers nothing that turns it into a `DType`.
        let c =
            ModelConfigMetadata::parse("config.json", &fixture_config_text("tiny-llama-2shard"))
                .unwrap();
        assert_eq!(c.torch_dtype.as_deref(), Some("float32"));
        let wire = serde_json::to_value(&c).unwrap();
        assert_eq!(wire["torch_dtype"], serde_json::json!("float32"));
        // An unrecognized spelling is kept, not mapped onto a known width.
        let odd =
            ModelConfigMetadata::parse("config.json", r#"{"torch_dtype": "float4_e2m1"}"#).unwrap();
        assert_eq!(odd.torch_dtype.as_deref(), Some("float4_e2m1"));
    }

    #[test]
    fn observed_layer_count_wins_and_declared_fills_in_only_an_absence() {
        let c = ModelConfigMetadata::parse("config.json", r#"{"num_hidden_layers": 12}"#).unwrap();
        // Observed comes from the shard headers by way of resolution and is
        // exact, so it wins even when the config disagrees.
        assert_eq!(c.layer_count(Some(3)), Some(3));
        // Nothing observed — the generic-fallback case, where no resolver set a
        // layer index — so the declared value is reported rather than NULL.
        assert_eq!(c.layer_count(None), Some(12));
        // Neither observed nor declared stays absent; it is not invented.
        assert_eq!(ModelConfigMetadata::default().layer_count(None), None);
    }

    #[test]
    fn builtin_registry_loads_every_manifest() {
        let r = Registry::builtin().unwrap();
        assert_eq!(r.plugins().len(), 5);
        assert!(r.get("llama").unwrap().is_implemented());
        assert!(r.get("generic").unwrap().is_implemented());
    }

    #[test]
    fn llama_is_selected_by_model_type_and_by_architecture() {
        let r = Registry::builtin().unwrap();
        assert_eq!(r.select(Some("llama"), None).unwrap().id(), "llama");
        assert_eq!(
            r.select(None, Some("LlamaForCausalLM")).unwrap().id(),
            "llama"
        );
        assert!(r.select(Some("llama"), None).unwrap().matched);
    }

    #[test]
    fn unknown_model_falls_back_to_generic() {
        let r = Registry::builtin().unwrap();
        let sel = r
            .select(Some("some_new_family"), Some("XForCausalLM"))
            .unwrap();
        assert_eq!(sel.id(), "generic");
        assert!(!sel.matched);
    }

    #[test]
    fn llama_declares_the_architecture_md_aliases() {
        let r = Registry::builtin().unwrap();
        let map = r.get("llama").unwrap().alias_map();
        assert_eq!(map["Q"], vec!["attention_query_projection"]);
        // `Att` is ambiguous by design (ARCHITECTURE.md §6.2).
        assert_eq!(map["Att"].len(), 4);
        assert!(map.contains_key("MLP.down"));
        assert!(map.contains_key("Expert.up"));
    }

    #[test]
    fn generic_declares_no_aliases_and_claims_nothing() {
        let r = Registry::builtin().unwrap();
        let g = r.get("generic").unwrap();
        assert!(g.aliases.is_empty());
        assert!(!g.claims(Some("llama"), None));
        assert!(g.match_spec.model_types.is_empty());
    }

    #[test]
    fn loading_from_the_repo_directory_matches_the_builtins() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../architectures");
        let from_disk = Registry::load_dir(dir).unwrap();
        let builtin = Registry::builtin().unwrap();
        assert_eq!(from_disk.plugins().len(), builtin.plugins().len());
        assert_eq!(
            from_disk.get("llama").unwrap().rules.len(),
            builtin.get("llama").unwrap().rules.len()
        );
    }

    #[test]
    fn malformed_manifest_is_rejected_with_context() {
        let err = ArchitecturePlugin::parse("bad.toml", "not = [valid").unwrap_err();
        assert!(err.to_string().contains("bad.toml"));
    }

    #[test]
    fn expert_rules_use_the_expert_match_kind() {
        let r = Registry::builtin().unwrap();
        let llama = r.get("llama").unwrap();
        assert!(llama.rules.iter().any(|rule| {
            rule.match_kind == MatchKind::ExpertSuffix && rule.role == "moe_expert_down_projection"
        }));
        assert_eq!(llama.naming.expert_segment.as_deref(), Some("experts"));
    }

    // ------------------------------------------------------------------------
    // Qwen family — QM-0010, requirement NSIR-006.
    // ------------------------------------------------------------------------

    /// The Qwen fixture's `config.json`, read from disk. Only the two keys the
    /// registry selects on are extracted, because those are the only two the
    /// registry is allowed to look at.
    fn qwen_fixture_selection_keys() -> (Option<String>, Option<String>) {
        let text = fixture_config_text("tiny-qwen-single");
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let model_type = v["model_type"].as_str().map(str::to_string);
        let architecture = v["architectures"][0].as_str().map(str::to_string);
        (model_type, architecture)
    }

    #[test]
    fn qwen_is_selected_by_model_type_and_by_architecture() {
        // Acceptance criterion 4. The manifest declares two model types and
        // four architecture strings; each is asserted rather than sampled.
        let r = Registry::builtin().unwrap();
        for model_type in ["qwen2", "qwen3"] {
            let sel = r.select(Some(model_type), None).unwrap();
            assert_eq!(sel.id(), "qwen", "model_type {model_type}");
            assert!(sel.matched, "model_type {model_type}");
        }
        for architecture in [
            "Qwen2ForCausalLM",
            "Qwen3ForCausalLM",
            "Qwen2MoeForCausalLM",
            "Qwen3MoeForCausalLM",
        ] {
            let sel = r.select(None, Some(architecture)).unwrap();
            assert_eq!(sel.id(), "qwen", "architecture {architecture}");
            assert!(sel.matched, "architecture {architecture}");
        }
        // Priority: Qwen outranks the generic fallback rather than tying it.
        assert!(r.get("qwen").unwrap().plugin.priority > r.get("generic").unwrap().plugin.priority);
    }

    #[test]
    fn the_qwen_fixture_is_claimed_by_the_qwen_plugin_from_what_it_declares() {
        // The fixture's config.json is the input, so the selection is exercised
        // against a real file rather than against literals typed here.
        let r = Registry::builtin().unwrap();
        let (model_type, architecture) = qwen_fixture_selection_keys();
        assert_eq!(model_type.as_deref(), Some("qwen3"));
        assert_eq!(architecture.as_deref(), Some("Qwen3ForCausalLM"));
        let sel = r
            .select(model_type.as_deref(), architecture.as_deref())
            .unwrap();
        assert_eq!(sel.id(), "qwen");
        assert!(sel.matched);
    }

    #[test]
    fn builtin_registry_reports_qwen_as_implemented_with_rules_and_aliases() {
        // Acceptance criterion 1's structural half: `implemented = true` is a
        // claim, and a claim with no rule table behind it would be a lie.
        let r = Registry::builtin().unwrap();
        let qwen = r.get("qwen").unwrap();
        assert!(qwen.is_implemented());
        assert!(!qwen.rules.is_empty());
        assert!(!qwen.aliases.is_empty());
        assert_eq!(qwen.naming.stack, "language");
        assert_eq!(qwen.naming.layer_segment, "layers");
    }

    #[test]
    fn qwen_expert_rules_use_the_expert_match_kind() {
        // Acceptance criterion 6's structural half: `experts.N.*` addressing
        // needs both the expert segment and rules that match inside it.
        let r = Registry::builtin().unwrap();
        let qwen = r.get("qwen").unwrap();
        assert_eq!(qwen.naming.expert_segment.as_deref(), Some("experts"));
        for role in [
            "moe_expert_gate_projection",
            "moe_expert_up_projection",
            "moe_expert_down_projection",
        ] {
            assert!(
                qwen.rules
                    .iter()
                    .any(|rule| rule.match_kind == MatchKind::ExpertSuffix && rule.role == role),
                "no expert-suffix rule for {role}"
            );
        }
        // The router is a per-layer tensor, not a per-expert one.
        assert!(qwen
            .rules
            .iter()
            .any(|rule| { rule.role == "moe_router" && rule.match_kind == MatchKind::Suffix }));
    }

    #[test]
    fn qwen_declares_an_ambiguous_alias_alongside_the_unambiguous_ones() {
        // NSIR-007 is a property of the manifest before it is a property of
        // resolution: an alias with several roles is ambiguous *by declaration*.
        let r = Registry::builtin().unwrap();
        let map = r.get("qwen").unwrap().alias_map();
        assert_eq!(map["Q"], vec!["attention_query_projection"]);
        assert_eq!(map["Att"].len(), 4);
        assert_eq!(map["QKNorm"].len(), 2);
        for alias in ["K", "V", "O", "MLP.down", "Expert.up", "Router", "Head"] {
            assert!(map.contains_key(alias), "missing alias {alias}");
        }
    }

    #[test]
    fn unimplemented_plugins_are_declared_and_never_claim() {
        let r = Registry::builtin().unwrap();
        let mut unimpl = r.declared_but_unimplemented();
        unimpl.sort();
        // QM-0010 implemented Qwen; Kimi and DeepSeek stay declared-but-absent
        // by design (QM-0010 §Out of Scope, PRODUCT_SCOPE.md), and this test
        // still asserts that neither of them claims a model.
        assert_eq!(unimpl, vec!["deepseek", "kimi"]);
        for (model_type, architecture) in [
            (Some("kimi"), Some("KimiForCausalLM")),
            (Some("deepseek_v3"), Some("DeepseekV3ForCausalLM")),
        ] {
            let sel = r.select(model_type, architecture).unwrap();
            assert_eq!(sel.id(), "generic", "{model_type:?} was claimed");
            assert!(!sel.matched);
        }
        // The manifests still exist and still declare what they would match, so
        // the gap is visible rather than merely missing.
        assert!(!r.get("kimi").unwrap().match_spec.model_types.is_empty());
        assert!(r.get("kimi").unwrap().rules.is_empty());
        assert!(r.get("deepseek").unwrap().rules.is_empty());
    }

    #[test]
    fn a_kimi_model_falls_back_to_generic_and_kimi_does_not_claim_it() {
        // QM-0010 §Test Cases, row 7, asserted on its own: a `model_type` a
        // declared-but-unimplemented plugin names must still reach `generic`.
        let r = Registry::builtin().unwrap();
        let kimi = r.get("kimi").unwrap();
        assert!(kimi.match_spec.model_types.iter().any(|m| m == "kimi"));
        assert!(!kimi.claims(Some("kimi"), None));
        let sel = r.select(Some("kimi"), None).unwrap();
        assert_eq!(sel.id(), "generic");
        assert!(!sel.matched);
    }

    #[test]
    fn no_plugin_rule_declares_more_axes_than_the_implemented_rank_ceiling() {
        // ADR-010: rank ≤ 3 is implemented and rank > 3 refuses rather than
        // flattens. Rank is not expressible in a *name* resolver — nothing here
        // sees a shape — so the nearest expressible surface is the number of
        // axis labels a rule declares. A rule declaring four would be a
        // manifest asserting a rank the rest of the system refuses to render.
        let r = Registry::builtin().unwrap();
        for plugin in r.plugins() {
            for rule in &plugin.rules {
                assert!(
                    rule.axes.len() <= 3,
                    "{}: rule `{}` declares {} axes; ADR-010 implements rank <= 3",
                    plugin.id(),
                    rule.name,
                    rule.axes.len()
                );
            }
        }
    }

    #[test]
    fn a_malformed_qwen_manifest_is_refused_at_load_naming_the_file() {
        // QM-0010 §Error Handling. Truncating the real manifest is a more
        // faithful corruption than an unrelated snippet.
        let mut text = BUILTIN_QWEN.to_string();
        text.truncate(text.len() / 2);
        text.push_str("\n[[rules]]\nname = \n");
        let err = ArchitecturePlugin::parse("architectures/qwen/plugin.toml", &text).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("architectures/qwen/plugin.toml"), "{msg}");
        // A manifest missing its identity is refused rather than defaulted into
        // an anonymous plugin.
        let err = ArchitecturePlugin::parse("qwen.toml", "[match]\nmodel_types = [\"qwen3\"]\n")
            .unwrap_err();
        assert!(err.to_string().contains("qwen.toml"));
    }

    #[test]
    fn the_qwen_fixture_config_declares_every_field_this_type_records() {
        // Hand-read from fixtures/tiny-qwen-single/config.json.
        let c = ModelConfigMetadata::parse("config.json", &fixture_config_text("tiny-qwen-single"))
            .unwrap();
        assert_eq!(c.hidden_size, Some(48));
        assert_eq!(c.num_hidden_layers, Some(12));
        assert_eq!(c.intermediate_size, Some(64));
        assert_eq!(c.num_attention_heads, Some(8));
        assert_eq!(c.num_key_value_heads, Some(2));
        assert_eq!(c.vocab_size, Some(64));
        assert_eq!(c.torch_dtype.as_deref(), Some("bfloat16"));
        assert!(!c.is_empty());
    }

    #[test]
    fn a_qwen_config_field_that_is_absent_is_never_inferred_from_its_neighbours() {
        // `num_attention_heads * head_dim == 128`, which is not this model's
        // hidden_size (48); `num_experts` says nothing about `num_hidden_layers`.
        let c = ModelConfigMetadata::parse(
            "config.json",
            r#"{"model_type": "qwen3_moe", "num_attention_heads": 8, "head_dim": 128,
                "num_experts": 128, "num_experts_per_tok": 8}"#,
        )
        .unwrap();
        assert_eq!(c.hidden_size, None);
        assert_eq!(c.num_hidden_layers, None);
        assert_eq!(c.intermediate_size, None);
        assert_eq!(c.num_key_value_heads, None);
        assert_eq!(c.num_attention_heads, Some(8));
    }

    #[test]
    fn a_malformed_qwen_config_json_is_refused_with_the_file_named() {
        let err = ModelConfigMetadata::parse(
            "fixtures/tiny-qwen-single/config.json",
            r#"{"model_type": "qwen3", "hidden_size": 48,"#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("fixtures/tiny-qwen-single/config.json"),
            "{msg}"
        );
        assert!(matches!(err, QError::Json { .. }), "{err:?}");
    }
}
