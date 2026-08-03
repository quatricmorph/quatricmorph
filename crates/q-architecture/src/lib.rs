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
//! family needs a structural concept the schema cannot express.

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
    /// The built-in registry: `generic` + `llama` implemented, the rest
    /// declared-but-unimplemented.
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

    #[test]
    fn builtin_registry_loads_every_manifest() {
        let r = Registry::builtin().unwrap();
        assert_eq!(r.plugins().len(), 5);
        assert!(r.get("llama").unwrap().is_implemented());
        assert!(r.get("generic").unwrap().is_implemented());
    }

    #[test]
    fn unimplemented_plugins_are_declared_and_never_claim() {
        let r = Registry::builtin().unwrap();
        let mut unimpl = r.declared_but_unimplemented();
        unimpl.sort();
        assert_eq!(unimpl, vec!["deepseek", "kimi", "qwen"]);
        // Qwen declares model_type "qwen2" but is not implemented, so a qwen2
        // model falls back to generic rather than being silently mis-resolved.
        let sel = r.select(Some("qwen2"), Some("Qwen2ForCausalLM")).unwrap();
        assert_eq!(sel.id(), "generic");
        assert!(!sel.matched);
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
}
