//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §4.2, §6).
//!
//! The NSIR compiler: raw tensor names → structured records → canonical
//! addresses, plus alias resolution with candidate lists.
//!
//! ## Order of operations
//!
//! Matching ARCHITECTURE.md §3's pipeline:
//!
//! ```text
//! SafeTensors ingestion  ->  TensorDescriptor (raw_name, shape, dtype, bytes)
//! Architecture resolver  ->  which plugin claims this model
//! NSIR compiler          ->  NsirRecord + canonical_name  (this module)
//! ```
//!
//! ## The never-guess rule, concretely
//!
//! Resolution reads **names only**. `resolve_name` never sees a shape. If a
//! name is not in the plugin's rule table, the result is
//! [`NsirRecord::unknown`] and the canonical name falls back to the raw name —
//! which remains a perfectly good stable address, just an unresolved one.

use crate::address::{CanonicalAddress, ElementSelector};
use crate::alias::ParsedAlias;
use crate::record::NsirRecord;
use q_architecture::{ArchitecturePlugin, MatchKind, Registry, Rule};
use q_source::error::{QError, Result};
use q_source::role::{Component, Stack, TensorRole};
use q_source::TensorDescriptor;
use serde::{Deserialize, Serialize};

/// Structure a resolver can read off a raw name without any semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NameStructure<'a> {
    layer: Option<u32>,
    expert: Option<u32>,
    /// Name with the `…layers.N.` (and `…experts.M.`) prefixes removed.
    suffix: &'a str,
    /// Trailing `.weight` / `.bias` segment, or `""`.
    parameter: &'a str,
}

fn split_structure<'a>(
    raw: &'a str,
    layer_segment: &str,
    expert_segment: Option<&str>,
) -> NameStructure<'a> {
    let mut layer = None;
    let mut expert = None;
    let mut suffix = raw;

    // `<prefix>.<layer_segment>.<N>.<rest>`
    let layer_marker = format!(".{layer_segment}.");
    if let Some(pos) = raw.find(&layer_marker) {
        let rest = &raw[pos + layer_marker.len()..];
        if let Some(dot) = rest.find('.') {
            if let Ok(n) = rest[..dot].parse::<u32>() {
                layer = Some(n);
                suffix = &rest[dot + 1..];
            }
        }
    }

    if let Some(seg) = expert_segment {
        let expert_marker = format!("{seg}.");
        // Search within the post-layer suffix so `experts.37.` is found even
        // when nested under `mlp.`.
        if let Some(pos) = suffix.find(&expert_marker) {
            let rest = &suffix[pos + expert_marker.len()..];
            if let Some(dot) = rest.find('.') {
                if let Ok(n) = rest[..dot].parse::<u32>() {
                    expert = Some(n);
                    suffix = &rest[dot + 1..];
                }
            }
        }
    }

    let parameter = match suffix.rfind('.') {
        Some(i) => &suffix[i + 1..],
        None => suffix,
    };
    let parameter = if parameter == "weight" || parameter == "bias" {
        parameter
    } else {
        ""
    };

    NameStructure {
        layer,
        expert,
        suffix,
        parameter,
    }
}

fn rule_matches(rule: &Rule, raw: &str, s: &NameStructure<'_>) -> bool {
    match rule.match_kind {
        MatchKind::Exact => raw == rule.name,
        MatchKind::Suffix => s.layer.is_some() && s.expert.is_none() && s.suffix == rule.name,
        MatchKind::ExpertSuffix => s.expert.is_some() && s.suffix == rule.name,
    }
}

/// The canonical path segment used for a component in a canonical address.
fn component_segment(component: Component) -> &'static str {
    match component {
        Component::Attention => "self_attention",
        Component::Mlp => "mlp",
        Component::MoE => "moe",
        Component::Normalization => "normalization",
        Component::Router => "router",
        Component::Embedding => "embedding",
        Component::OutputHead => "output_head",
        Component::Unknown => "unknown",
    }
}

/// Build the canonical address string for a resolved record.
///
/// Shape follows ARCHITECTURE.md §6.1:
/// `model.layers[10].self_attention.query_projection.weight`.
pub fn canonical_name(record: &NsirRecord) -> Option<String> {
    if !record.resolved {
        return None;
    }
    let segment = component_segment(record.component);
    let mut s = String::from("model");
    if let Some(layer) = record.layer {
        s.push_str(&format!(".layers[{layer}]"));
    }
    s.push('.');
    s.push_str(segment);
    if let Some(expert) = record.expert {
        s.push_str(&format!(".experts[{expert}]"));
    }
    s.push('.');
    s.push_str(&record.operation);
    if !record.parameter.is_empty() {
        s.push('.');
        s.push_str(&record.parameter);
    }
    Some(s)
}

/// One architecture plugin, ready to resolve names.
pub struct NsirResolver<'a> {
    plugin: &'a ArchitecturePlugin,
}

impl<'a> NsirResolver<'a> {
    pub fn new(plugin: &'a ArchitecturePlugin) -> Self {
        Self { plugin }
    }

    pub fn id(&self) -> &str {
        self.plugin.id()
    }

    /// Resolve one raw tensor name. **Never sees a shape.**
    pub fn resolve_name(&self, raw: &str) -> NsirRecord {
        let naming = &self.plugin.naming;
        let s = split_structure(raw, &naming.layer_segment, naming.expert_segment.as_deref());

        for rule in &self.plugin.rules {
            if rule_matches(rule, raw, &s) {
                let role = TensorRole::parse(&rule.role);
                let component = rule
                    .component
                    .as_deref()
                    .map(Component::parse)
                    .unwrap_or_else(|| role.component());
                return NsirRecord {
                    stack: Stack::parse(&naming.stack),
                    layer: s.layer,
                    component,
                    expert: s.expert,
                    operation: rule.operation.clone(),
                    parameter: rule.parameter.clone(),
                    axes: rule.axes.clone(),
                    role,
                    resolver_id: self.plugin.id().to_string(),
                    resolved: true,
                };
            }
        }
        NsirRecord::unknown(self.plugin.id(), s.layer, s.parameter)
    }

    /// Annotate a descriptor in place with canonical name, role, and layer.
    ///
    /// Unresolved descriptors keep `canonical_name == raw_name` and role
    /// `Unknown`. That is a usable address, honestly labelled.
    pub fn annotate(&self, descriptor: &mut TensorDescriptor) -> NsirRecord {
        let record = self.resolve_name(&descriptor.raw_name);
        if let Some(name) = canonical_name(&record) {
            descriptor.canonical_name = name;
        } else {
            descriptor.canonical_name = descriptor.raw_name.clone();
        }
        descriptor.semantic_role = record.role;
        descriptor.layer_index = record.layer;
        record
    }
}

/// One tensor an alias could refer to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AliasCandidate {
    pub raw_name: String,
    pub canonical_name: String,
    pub role: TensorRole,
    pub layer_index: Option<u32>,
    pub expert_index: Option<u32>,
    pub shape: Vec<u64>,
}

/// The result of resolving a contextual alias (ARCHITECTURE.md §6.2).
///
/// When `candidates.len() > 1` the alias is ambiguous and the caller must
/// choose — the resolver will not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AliasResolution {
    pub input: String,
    pub alias: String,
    pub candidates: Vec<AliasCandidate>,
    pub selector: Option<ElementSelector>,
    /// 1.0 for a unique match, 0.0 when nothing matched, `1/n` when ambiguous.
    pub confidence: f32,
}

impl AliasResolution {
    pub fn is_ambiguous(&self) -> bool {
        self.candidates.len() > 1
    }

    /// The single candidate, or an error naming all of them.
    pub fn unique(&self) -> Result<&AliasCandidate> {
        match self.candidates.len() {
            1 => Ok(&self.candidates[0]),
            0 => Err(QError::NotFound(format!(
                "alias `{}` matched no tensor",
                self.input
            ))),
            _ => Err(QError::AmbiguousAlias {
                alias: self.input.clone(),
                candidates: self
                    .candidates
                    .iter()
                    .map(|c| c.canonical_name.clone())
                    .collect(),
            }),
        }
    }
}

/// A resolved model: descriptors plus the plugin that explained them.
pub struct ResolvedModel {
    pub resolver_id: String,
    pub descriptors: Vec<TensorDescriptor>,
    alias_map: std::collections::BTreeMap<String, Vec<String>>,
}

impl ResolvedModel {
    /// Resolve every descriptor with the plugin selected for this model.
    pub fn build(
        registry: &Registry,
        model_type: Option<&str>,
        architecture: Option<&str>,
        mut descriptors: Vec<TensorDescriptor>,
    ) -> Result<Self> {
        let selection = registry.select(model_type, architecture)?;
        let resolver = NsirResolver::new(selection.plugin);
        for d in &mut descriptors {
            resolver.annotate(d);
        }
        Ok(Self {
            resolver_id: selection.id().to_string(),
            descriptors,
            alias_map: selection.plugin.alias_map(),
        })
    }

    pub fn unresolved_count(&self) -> usize {
        self.descriptors
            .iter()
            .filter(|d| d.semantic_role == TensorRole::Unknown)
            .count()
    }

    pub fn by_canonical_name(&self, name: &str) -> Option<&TensorDescriptor> {
        self.descriptors.iter().find(|d| d.canonical_name == name)
    }

    pub fn by_raw_name(&self, name: &str) -> Option<&TensorDescriptor> {
        self.descriptors.iter().find(|d| d.raw_name == name)
    }

    /// Resolve a canonical address (with or without an element selector).
    pub fn resolve_canonical(
        &self,
        address: &str,
    ) -> Result<(&TensorDescriptor, Option<ElementSelector>)> {
        let parsed = CanonicalAddress::parse(address)?;
        let path = parsed.tensor_path();
        let d = self
            .by_canonical_name(&path)
            .or_else(|| self.by_raw_name(&path))
            .or_else(|| self.by_raw_name(address))
            .ok_or_else(|| QError::NotFound(format!("no tensor at canonical address `{path}`")))?;
        Ok((d, parsed.selector))
    }

    /// Resolve a contextual alias to candidates (ARCHITECTURE.md §6.2).
    pub fn resolve_alias(&self, input: &str) -> Result<AliasResolution> {
        let parsed = ParsedAlias::parse(input)?;
        let roles = self.alias_map.get(&parsed.alias).ok_or_else(|| {
            QError::QueryRejected(format!(
                "unknown alias `{}`; this model was resolved by the `{}` plugin, which declares {} aliases",
                parsed.alias,
                self.resolver_id,
                self.alias_map.len()
            ))
        })?;

        let wanted: Vec<TensorRole> = roles.iter().map(|r| TensorRole::parse(r)).collect();
        let mut candidates: Vec<AliasCandidate> = Vec::new();
        // Iterate roles outermost so candidate order follows the plugin's
        // declared order (Q, K, V, O), which is what a user sees in the
        // ambiguity message.
        for role in &wanted {
            for d in &self.descriptors {
                if d.semantic_role != *role {
                    continue;
                }
                if parsed.layer_index.is_some() && d.layer_index != parsed.layer_index {
                    continue;
                }
                if let Some(expert) = parsed.expert_index {
                    let addr = CanonicalAddress::parse(&d.canonical_name).ok();
                    if addr.and_then(|a| a.expert_index()) != Some(expert) {
                        continue;
                    }
                }
                candidates.push(AliasCandidate {
                    raw_name: d.raw_name.clone(),
                    canonical_name: d.canonical_name.clone(),
                    role: d.semantic_role,
                    layer_index: d.layer_index,
                    expert_index: CanonicalAddress::parse(&d.canonical_name)
                        .ok()
                        .and_then(|a| a.expert_index()),
                    shape: d.shape.clone(),
                });
            }
        }

        let confidence = match candidates.len() {
            0 => 0.0,
            1 => 1.0,
            n => 1.0 / n as f32,
        };
        Ok(AliasResolution {
            input: parsed.input,
            alias: parsed.alias,
            candidates,
            selector: parsed.selector,
            confidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use q_source::{DType, ModelId, TensorId};

    fn descriptor(raw: &str, shape: Vec<u64>) -> TensorDescriptor {
        let model = ModelId::derive("m", "", "f");
        let n: u64 = shape.iter().product();
        TensorDescriptor {
            tensor_id: TensorId::derive(model, raw),
            raw_name: raw.to_string(),
            canonical_name: raw.to_string(),
            shape,
            dtype: DType::F32,
            shard_uri: "s.safetensors".into(),
            byte_start: 0,
            byte_end: n * 4,
            layer_index: None,
            semantic_role: TensorRole::Unknown,
        }
    }

    fn llama_resolver(reg: &Registry) -> NsirResolver<'_> {
        NsirResolver::new(reg.get("llama").unwrap())
    }

    #[test]
    fn llama_resolves_the_architecture_md_example() {
        let reg = Registry::builtin().unwrap();
        let r = llama_resolver(&reg).resolve_name("model.layers.10.self_attn.q_proj.weight");
        assert!(r.resolved);
        assert_eq!(r.stack, Stack::Language);
        assert_eq!(r.layer, Some(10));
        assert_eq!(r.component, Component::Attention);
        assert_eq!(r.operation, "query_projection");
        assert_eq!(r.parameter, "weight");
        assert_eq!(r.axes, vec!["output_channel", "input_channel"]);
        assert_eq!(r.role, TensorRole::AttentionQueryProjection);
        assert_eq!(
            canonical_name(&r).unwrap(),
            "model.layers[10].self_attention.query_projection.weight"
        );
    }

    #[test]
    fn llama_resolves_moe_expert_tensors() {
        let reg = Registry::builtin().unwrap();
        let r =
            llama_resolver(&reg).resolve_name("model.layers.10.mlp.experts.37.down_proj.weight");
        assert!(r.resolved);
        assert_eq!(r.layer, Some(10));
        assert_eq!(r.expert, Some(37));
        assert_eq!(r.component, Component::MoE);
        assert_eq!(r.operation, "down_projection");
        assert_eq!(
            canonical_name(&r).unwrap(),
            "model.layers[10].moe.experts[37].down_projection.weight"
        );
    }

    #[test]
    fn generic_resolver_returns_unknown_for_names_it_was_not_taught() {
        let reg = Registry::builtin().unwrap();
        let g = NsirResolver::new(reg.get("generic").unwrap());
        // A vision-tower name from a multimodal checkpoint: structurally a
        // tensor, semantically unrecognised.
        let r = g.resolve_name("visual.blocks.3.attn.qkv.weight");
        assert!(!r.resolved);
        assert_eq!(r.role, TensorRole::Unknown);
        assert_eq!(r.component, Component::Unknown);
        assert!(r.axes.is_empty());
        assert!(canonical_name(&r).is_none());
    }

    #[test]
    fn generic_resolver_still_handles_the_universal_convention() {
        let reg = Registry::builtin().unwrap();
        let g = NsirResolver::new(reg.get("generic").unwrap());
        let r = g.resolve_name("model.layers.4.mlp.down_proj.weight");
        assert!(r.resolved);
        assert_eq!(r.role, TensorRole::MlpDownProjection);
        assert_eq!(r.layer, Some(4));
    }

    #[test]
    fn generic_resolver_has_no_moe_rules_and_says_so() {
        let reg = Registry::builtin().unwrap();
        let g = NsirResolver::new(reg.get("generic").unwrap());
        let r = g.resolve_name("model.layers.10.mlp.experts.37.down_proj.weight");
        assert!(!r.resolved);
        assert_eq!(r.role, TensorRole::Unknown);
    }

    #[test]
    fn unresolved_names_keep_the_raw_name_as_their_address() {
        let reg = Registry::builtin().unwrap();
        let g = NsirResolver::new(reg.get("generic").unwrap());
        let mut d = descriptor("visual.blocks.3.attn.qkv.weight", vec![4, 4]);
        let r = g.annotate(&mut d);
        assert!(!r.resolved);
        assert_eq!(d.canonical_name, "visual.blocks.3.attn.qkv.weight");
        assert_eq!(d.semantic_role, TensorRole::Unknown);
    }

    #[test]
    fn canonical_names_are_stable_across_resolution_runs() {
        let reg = Registry::builtin().unwrap();
        let r = llama_resolver(&reg);
        let a = r.resolve_name("model.layers.10.self_attn.q_proj.weight");
        let b = r.resolve_name("model.layers.10.self_attn.q_proj.weight");
        assert_eq!(canonical_name(&a), canonical_name(&b));
    }

    fn tiny_model() -> ResolvedModel {
        let reg = Registry::builtin().unwrap();
        let mut ds = Vec::new();
        for layer in [10u32, 20] {
            for (suffix, shape) in [
                ("self_attn.q_proj.weight", vec![128u64, 48]),
                ("self_attn.k_proj.weight", vec![32, 48]),
                ("self_attn.v_proj.weight", vec![32, 48]),
                ("self_attn.o_proj.weight", vec![48, 128]),
                ("mlp.down_proj.weight", vec![48, 64]),
            ] {
                ds.push(descriptor(&format!("model.layers.{layer}.{suffix}"), shape));
            }
        }
        ds.push(descriptor("model.embed_tokens.weight", vec![64, 48]));
        ds.push(descriptor("visual.blocks.0.attn.qkv.weight", vec![4, 4]));
        ResolvedModel::build(&reg, Some("llama"), None, ds).unwrap()
    }

    #[test]
    fn alias_q_resolves_uniquely() {
        let m = tiny_model();
        let r = m.resolve_alias("Q[10]").unwrap();
        assert!(!r.is_ambiguous());
        assert_eq!(r.confidence, 1.0);
        let c = r.unique().unwrap();
        assert_eq!(c.raw_name, "model.layers.10.self_attn.q_proj.weight");
        assert_eq!(
            c.canonical_name,
            "model.layers[10].self_attention.query_projection.weight"
        );
    }

    #[test]
    fn ambiguous_alias_returns_candidates_not_a_silent_pick() {
        let m = tiny_model();
        let r = m.resolve_alias("Att[10]").unwrap();
        assert!(r.is_ambiguous());
        assert_eq!(r.candidates.len(), 4);
        assert_eq!(r.confidence, 0.25);
        // Candidate order follows the plugin's declared role order: Q, K, V, O.
        let roles: Vec<TensorRole> = r.candidates.iter().map(|c| c.role).collect();
        assert_eq!(
            roles,
            vec![
                TensorRole::AttentionQueryProjection,
                TensorRole::AttentionKeyProjection,
                TensorRole::AttentionValueProjection,
                TensorRole::AttentionOutputProjection,
            ]
        );
        match r.unique() {
            Err(QError::AmbiguousAlias { candidates, .. }) => assert_eq!(candidates.len(), 4),
            other => panic!("expected AmbiguousAlias, got {other:?}"),
        }
    }

    #[test]
    fn alias_carries_its_element_selector_through() {
        let m = tiny_model();
        let r = m.resolve_alias("Q[10][100,42]").unwrap();
        let sel = r.selector.as_ref().unwrap();
        assert!(sel.is_scalar_for(&r.unique().unwrap().shape));
        assert_eq!(sel.as_point_index(&[128, 48]).unwrap(), vec![100, 42]);
    }

    #[test]
    fn alias_without_a_layer_matches_every_layer() {
        let m = tiny_model();
        let r = m.resolve_alias("Q").unwrap();
        assert_eq!(r.candidates.len(), 2); // layers 10 and 20
        assert!(r.is_ambiguous());
    }

    #[test]
    fn alias_for_a_missing_layer_matches_nothing() {
        let m = tiny_model();
        let r = m.resolve_alias("Q[99]").unwrap();
        assert!(r.candidates.is_empty());
        assert_eq!(r.confidence, 0.0);
        assert!(matches!(r.unique(), Err(QError::NotFound(_))));
    }

    #[test]
    fn unknown_alias_is_rejected_with_an_explanation() {
        let m = tiny_model();
        let err = m.resolve_alias("Zzz[10]").unwrap_err();
        assert!(err.to_string().contains("unknown alias"));
    }

    #[test]
    fn mlp_dotted_alias_resolves() {
        let m = tiny_model();
        let c = m.resolve_alias("MLP.down[20]").unwrap();
        assert_eq!(
            c.unique().unwrap().raw_name,
            "model.layers.20.mlp.down_proj.weight"
        );
    }

    #[test]
    fn canonical_address_lookup_works_and_reports_unresolved_tensors() {
        let m = tiny_model();
        let (d, sel) = m
            .resolve_canonical("model.layers[10].self_attention.query_projection.weight[100,42]")
            .unwrap();
        assert_eq!(d.raw_name, "model.layers.10.self_attn.q_proj.weight");
        assert!(sel.unwrap().is_scalar_for(&d.shape));

        // The vision tensor is unresolved, so its raw name is its address.
        assert_eq!(m.unresolved_count(), 1);
        let (d2, _) = m
            .resolve_canonical("visual.blocks.0.attn.qkv.weight")
            .unwrap();
        assert_eq!(d2.semantic_role, TensorRole::Unknown);
    }

    #[test]
    fn canonical_address_for_a_missing_tensor_is_not_found() {
        let m = tiny_model();
        assert!(matches!(
            m.resolve_canonical("model.layers[77].self_attention.query_projection.weight"),
            Err(QError::NotFound(_))
        ));
    }

    #[test]
    fn structure_splitting_finds_layer_and_expert() {
        let s = split_structure(
            "model.layers.10.mlp.experts.37.down_proj.weight",
            "layers",
            Some("experts"),
        );
        assert_eq!(s.layer, Some(10));
        assert_eq!(s.expert, Some(37));
        assert_eq!(s.suffix, "down_proj.weight");
        assert_eq!(s.parameter, "weight");
    }

    #[test]
    fn structure_splitting_leaves_non_layer_names_alone() {
        let s = split_structure("model.embed_tokens.weight", "layers", Some("experts"));
        assert_eq!(s.layer, None);
        assert_eq!(s.suffix, "model.embed_tokens.weight");
        assert_eq!(s.parameter, "weight");
    }

    // ------------------------------------------------------------------------
    // Qwen family — QM-0010, requirement NSIR-006.
    //
    // Every expectation below comes from one of two places, and never from
    // running this resolver:
    //
    //   * `fixtures/tiny-qwen-single/golden.json`, whose rows
    //     `fixtures/generate_fixtures.py` writes out by hand from the address
    //     rule in ARCHITECTURE.md §6.1; or
    //   * an inline string literal in the test body, written from the same rule.
    //
    // Both are asserted, deliberately: if the manifest and the fixture ever
    // drift together, the inline literals still fail.
    // ------------------------------------------------------------------------

    fn qwen_resolver(reg: &Registry) -> NsirResolver<'_> {
        NsirResolver::new(reg.get("qwen").unwrap())
    }

    /// `fixtures/tiny-qwen-single/golden.json`, read from disk rather than
    /// duplicated here, so the expectations are checked against the real file.
    fn qwen_golden() -> serde_json::Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/tiny-qwen-single/golden.json");
        let text = std::fs::read_to_string(&path).expect("run fixtures/generate_fixtures.py");
        serde_json::from_str(&text).expect("golden.json is not valid JSON")
    }

    fn golden_rows(golden: &serde_json::Value, key: &str) -> Vec<serde_json::Value> {
        golden[key]
            .as_array()
            .unwrap_or_else(|| panic!("golden.json has no `{key}` array"))
            .clone()
    }

    fn s(value: &serde_json::Value) -> &str {
        value.as_str().expect("expected a JSON string")
    }

    #[test]
    fn qwen_resolves_every_fixture_name_to_its_hand_written_canonical_address() {
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        let golden = qwen_golden();
        // A names-only fixture: asserting this here keeps the claim honest if
        // someone later adds weights to the directory.
        assert_eq!(golden["carries_weight_payload"], serde_json::json!(false));
        assert_eq!(golden["resolver_id"], serde_json::json!("qwen"));

        let rows = golden_rows(&golden, "resolved");
        assert!(!rows.is_empty(), "golden.json resolved no names");
        for row in &rows {
            let raw = s(&row["raw_name"]);
            let got = r.resolve_name(raw);
            assert!(got.resolved, "{raw} did not resolve");
            assert_eq!(got.stack, Stack::Language, "{raw} stack");
            assert_eq!(got.resolver_id, "qwen", "{raw} resolver_id");
            assert_eq!(got.role.as_str(), s(&row["role"]), "{raw} role");
            assert_eq!(
                got.component.as_str(),
                s(&row["component"]),
                "{raw} component"
            );
            assert_eq!(got.operation, s(&row["operation"]), "{raw} operation");
            assert_eq!(got.parameter, s(&row["parameter"]), "{raw} parameter");
            assert_eq!(
                serde_json::to_value(&got.axes).unwrap(),
                row["axes"],
                "{raw} axes"
            );
            assert_eq!(
                serde_json::to_value(got.layer).unwrap(),
                row["layer"],
                "{raw} layer"
            );
            assert_eq!(
                serde_json::to_value(got.expert).unwrap(),
                row["expert"],
                "{raw} expert"
            );
            assert_eq!(
                canonical_name(&got).unwrap(),
                s(&row["canonical_name"]),
                "{raw} canonical address"
            );
        }
    }

    #[test]
    fn qwen_resolves_all_fifteen_name_families_named_in_the_task_scope() {
        let golden = qwen_golden();
        // Acceptance criterion 1 counts families, so the count is asserted
        // rather than inferred from however many rows happen to be present.
        let families: Vec<&str> = golden["name_families"]
            .as_array()
            .unwrap()
            .iter()
            .map(s)
            .collect();
        assert_eq!(families.len(), 15, "QM-0010 §Scope names 15 families");
        assert_eq!(golden["name_family_count"], serde_json::json!(15));

        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        let rows = golden_rows(&golden, "resolved");
        for family in &families {
            let mut covered = rows
                .iter()
                .filter(|row| s(&row["family"]) == *family)
                .peekable();
            assert!(
                covered.peek().is_some(),
                "no fixture row covers the `{family}` family"
            );
            for row in covered {
                let raw = s(&row["raw_name"]);
                assert!(
                    r.resolve_name(raw).resolved,
                    "family `{family}`: {raw} did not resolve"
                );
            }
        }
    }

    #[test]
    fn a_qwen_name_the_resolver_was_not_taught_stays_unknown() {
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        let golden = qwen_golden();
        let untaught = golden_rows(&golden, "untaught");
        assert!(!untaught.is_empty(), "golden.json lists no untaught names");
        for row in &untaught {
            let raw = s(&row["raw_name"]);
            let got = r.resolve_name(raw);
            assert!(!got.resolved, "{raw} claimed to resolve: {got:?}");
            assert_eq!(got.role, TensorRole::Unknown, "{raw} role");
            assert_eq!(got.component, Component::Unknown, "{raw} component");
            assert!(got.operation.is_empty(), "{raw} invented an operation");
            assert!(got.axes.is_empty(), "{raw} invented axis labels");
            assert!(
                canonical_name(&got).is_none(),
                "{raw} produced a canonical address it has not earned"
            );
        }
        // The task's own §Test Cases row, asserted directly rather than only
        // through the fixture loop.
        let future = r.resolve_name("model.layers.0.some_future_thing.weight");
        assert!(!future.resolved);
        assert_eq!(future.role, TensorRole::Unknown);
    }

    #[test]
    fn a_non_numeric_or_out_of_range_layer_index_leaves_the_layer_absent() {
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        for raw in [
            "model.layers.abc.self_attn.q_proj.weight",
            // One past u32::MAX. Out of range is absent, never wrapped.
            "model.layers.4294967296.self_attn.q_proj.weight",
            "model.layers.-1.self_attn.q_proj.weight",
        ] {
            let got = r.resolve_name(raw);
            assert_eq!(got.layer, None, "{raw} invented a layer index");
            assert!(!got.resolved, "{raw} resolved without a layer index");
            assert_eq!(got.role, TensorRole::Unknown, "{raw} role");
        }
        // The in-range neighbour does resolve, so the three refusals above are
        // about the index and not about the rest of the name.
        let ok = r.resolve_name("model.layers.4294967295.self_attn.q_proj.weight");
        assert_eq!(ok.layer, Some(u32::MAX));
        assert!(ok.resolved);
    }

    #[test]
    fn qwen_canonical_addresses_are_identical_in_form_to_llamas() {
        // Acceptance criterion 2. A canonical address is the universal join key
        // (.plan/DATA_ARCHITECTURE.md §4), so it must not depend on which
        // family produced the tensor. Every name both manifests are taught must
        // produce byte-identical output from both.
        let reg = Registry::builtin().unwrap();
        let llama = llama_resolver(&reg);
        let qwen = qwen_resolver(&reg);
        let golden = qwen_golden();
        let rows = golden_rows(&golden, "resolved");
        let mut compared = 0usize;
        let mut not_taught_to_llama: Vec<&str> = Vec::new();
        for row in &rows {
            let raw = s(&row["raw_name"]);
            let l = llama.resolve_name(raw);
            if !l.resolved {
                // Recorded, not skipped: a name only one manifest knows cannot
                // be compared, and a comparison that quietly covers less than it
                // claims is worse than one that fails.
                not_taught_to_llama.push(raw);
                continue;
            }
            let q = qwen.resolve_name(raw);
            assert_eq!(canonical_name(&l), canonical_name(&q), "{raw} address");
            assert_eq!(l.role, q.role, "{raw} role");
            assert_eq!(l.component, q.component, "{raw} component");
            assert_eq!(l.axes, q.axes, "{raw} axes");
            compared += 1;
        }
        // The Llama rule table is currently a superset of Qwen's suffixes — it
        // declares the q/k/v biases and the `q_norm`/`k_norm` rules even though
        // Llama checkpoints carry no such tensors — so nothing is skipped and
        // the comparison above is not vacuous. Should a Qwen-only tensor arrive
        // later, this names it instead of silently comparing fewer rows.
        assert!(
            not_taught_to_llama.is_empty(),
            "llama does not resolve {not_taught_to_llama:?}, so those rows were \
             not compared; decide whether the address form still has to match"
        );
        assert_eq!(
            compared,
            rows.len(),
            "compared {compared} of {} Qwen names",
            rows.len()
        );
    }

    #[test]
    fn qwen_resolves_the_task_data_contract_examples() {
        // QM-0010 §Data Contracts, with the three literals written from
        // ARCHITECTURE.md §6.1 and the declared manifests. See
        // .plan/evidence/QM-0010.md §Research: the task's abbreviated arrows
        // spell `expert[37]`, `query_norm`, and `moe.router`, which the Llama
        // form — normative per acceptance criterion 2 — renders `experts[37]`,
        // `query_normalization`, and `router.expert_routing`.
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);

        let q = r.resolve_name("model.layers.10.self_attn.q_proj.weight");
        assert_eq!(
            canonical_name(&q).unwrap(),
            "model.layers[10].self_attention.query_projection.weight"
        );
        assert_eq!(q.role, TensorRole::AttentionQueryProjection);
        assert_eq!(q.axes, vec!["output_channel", "input_channel"]);

        let up = r.resolve_name("model.layers.10.mlp.experts.37.up_proj.weight");
        assert_eq!(
            canonical_name(&up).unwrap(),
            "model.layers[10].moe.experts[37].up_projection.weight"
        );
        assert_eq!(up.role, TensorRole::MoeExpertUpProjection);
        assert_eq!(up.expert, Some(37));

        let qn = r.resolve_name("model.layers.10.self_attn.q_norm.weight");
        assert_eq!(
            canonical_name(&qn).unwrap(),
            "model.layers[10].self_attention.query_normalization.weight"
        );
        assert_eq!(qn.role, TensorRole::AttentionQueryNorm);
        assert_eq!(qn.component, Component::Attention);
    }

    #[test]
    fn qwen_moe_expert_addressing_uses_the_experts_n_layout() {
        // Acceptance criterion 6, over all three expert projections.
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        for (suffix, operation, role) in [
            (
                "gate_proj",
                "gate_projection",
                TensorRole::MoeExpertGateProjection,
            ),
            (
                "up_proj",
                "up_projection",
                TensorRole::MoeExpertUpProjection,
            ),
            (
                "down_proj",
                "down_projection",
                TensorRole::MoeExpertDownProjection,
            ),
        ] {
            let got = r.resolve_name(&format!("model.layers.3.mlp.experts.0.{suffix}.weight"));
            assert!(got.resolved, "{suffix}");
            assert_eq!(got.layer, Some(3), "{suffix} layer");
            assert_eq!(got.expert, Some(0), "{suffix} expert");
            assert_eq!(got.component, Component::MoE, "{suffix} component");
            assert_eq!(got.role, role, "{suffix} role");
            assert_eq!(
                canonical_name(&got).unwrap(),
                format!("model.layers[3].moe.experts[0].{operation}.weight"),
                "{suffix} address"
            );
        }
    }

    #[test]
    fn the_moe_router_and_the_dense_mlp_gate_are_different_roles() {
        // `mlp.gate.weight` and `mlp.gate_proj.weight` differ by five
        // characters and denote different objects: the first routes tokens to
        // experts, the second is one of the dense MLP's two input projections.
        // A resolver maps names, so it must keep them apart — and nothing about
        // either name or either shape reveals which is which. That is why the
        // mapping is declared in a manifest rather than inferred.
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);

        let router = r.resolve_name("model.layers.5.mlp.gate.weight");
        assert_eq!(router.role, TensorRole::MoeRouter);
        assert_eq!(router.component, Component::Router);
        assert_eq!(router.axes, vec!["expert", "hidden_channel"]);
        assert_eq!(
            canonical_name(&router).unwrap(),
            "model.layers[5].router.expert_routing.weight"
        );

        let dense = r.resolve_name("model.layers.5.mlp.gate_proj.weight");
        assert_eq!(dense.role, TensorRole::MlpGateProjection);
        assert_eq!(dense.component, Component::Mlp);
        assert_eq!(
            canonical_name(&dense).unwrap(),
            "model.layers[5].mlp.gate_projection.weight"
        );

        assert_ne!(router.role, dense.role);
        assert_ne!(canonical_name(&router), canonical_name(&dense));
    }

    #[test]
    fn qwen_reads_names_only_so_two_identically_shaped_tensors_get_different_roles() {
        // QM-0010 §Test Cases, last row. `resolve_name` takes a `&str`: a shape
        // is not merely unused here, it is unavailable — which is the strongest
        // available form of ARCHITECTURE.md §4.2's prohibition.
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        let shape = vec![64u64, 48];
        let mut a = descriptor("model.layers.7.self_attn.q_proj.weight", shape.clone());
        let mut b = descriptor("model.layers.7.mlp.up_proj.weight", shape.clone());
        assert_eq!(a.shape, b.shape, "the premise of this test");

        let ra = r.annotate(&mut a);
        let rb = r.annotate(&mut b);
        assert_eq!(ra.role, TensorRole::AttentionQueryProjection);
        assert_eq!(rb.role, TensorRole::MlpUpProjection);
        assert_ne!(ra.role, rb.role);
        assert_ne!(a.canonical_name, b.canonical_name);

        // And the converse: an untaught name stays unknown however ordinary its
        // shape looks next to a tensor that did resolve.
        let mut c = descriptor("model.layers.7.self_attn.qkv_proj.weight", shape);
        let rc = r.annotate(&mut c);
        assert_eq!(rc.role, TensorRole::Unknown);
        assert_eq!(c.canonical_name, "model.layers.7.self_attn.qkv_proj.weight");
    }

    #[test]
    fn qwen_biases_resolve_as_the_bias_parameter_of_their_projection() {
        // Qwen2 declares `attention_bias: true` and ships q/k/v biases; o_proj
        // has none, and Qwen3 has none at all. A bias is resolved by its name,
        // not by its rank.
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        for (proj, operation) in [
            ("q_proj", "query_projection"),
            ("k_proj", "key_projection"),
            ("v_proj", "value_projection"),
        ] {
            let got = r.resolve_name(&format!("model.layers.2.self_attn.{proj}.bias"));
            assert!(got.resolved, "{proj}.bias");
            assert_eq!(got.parameter, "bias", "{proj}.bias parameter");
            assert_eq!(got.axes, vec!["output_channel"], "{proj}.bias axes");
            assert_eq!(
                canonical_name(&got).unwrap(),
                format!("model.layers[2].self_attention.{operation}.bias")
            );
        }
        // Not taught, because Qwen does not ship it: unknown, not invented.
        let o = r.resolve_name("model.layers.2.self_attn.o_proj.bias");
        assert!(!o.resolved);
        assert_eq!(o.role, TensorRole::Unknown);
    }

    #[test]
    fn qwen_canonical_names_are_stable_across_resolution_runs() {
        // Acceptance criterion 7, over every name in the fixture rather than
        // one sample.
        let reg = Registry::builtin().unwrap();
        let r = qwen_resolver(&reg);
        for row in golden_rows(&qwen_golden(), "resolved") {
            let raw = s(&row["raw_name"]);
            let first = r.resolve_name(raw);
            let second = r.resolve_name(raw);
            assert_eq!(canonical_name(&first), canonical_name(&second), "{raw}");
            assert_eq!(first, second, "{raw} record");
        }
    }

    /// A Qwen-resolved model. Shapes are arbitrary and are never read by the
    /// resolver; they exist because `TensorDescriptor` has the field.
    fn tiny_qwen_model() -> ResolvedModel {
        let reg = Registry::builtin().unwrap();
        let mut ds = Vec::new();
        for layer in [10u32, 20] {
            for suffix in [
                "self_attn.q_proj.weight",
                "self_attn.k_proj.weight",
                "self_attn.v_proj.weight",
                "self_attn.o_proj.weight",
                "self_attn.q_norm.weight",
                "self_attn.k_norm.weight",
                "mlp.down_proj.weight",
            ] {
                ds.push(descriptor(
                    &format!("model.layers.{layer}.{suffix}"),
                    vec![16u64, 48],
                ));
            }
        }
        ds.push(descriptor("model.layers.10.mlp.gate.weight", vec![8, 48]));
        for expert in [0u32, 37] {
            ds.push(descriptor(
                &format!("model.layers.10.mlp.experts.{expert}.up_proj.weight"),
                vec![64, 48],
            ));
        }
        ds.push(descriptor("model.embed_tokens.weight", vec![64, 48]));
        // A Qwen2-MoE shared expert: a real name this plugin is not taught.
        ds.push(descriptor(
            "model.layers.10.mlp.shared_expert.up_proj.weight",
            vec![64, 48],
        ));
        ResolvedModel::build(&reg, Some("qwen3"), None, ds).unwrap()
    }

    #[test]
    fn a_qwen_model_is_resolved_by_the_qwen_plugin_and_reports_what_it_could_not_read() {
        let m = tiny_qwen_model();
        assert_eq!(m.resolver_id, "qwen");
        // Exactly the shared-expert tensor is unresolved, and it keeps its raw
        // name as its address rather than being filed under a routed expert.
        assert_eq!(m.unresolved_count(), 1);
        let (d, _) = m
            .resolve_canonical("model.layers.10.mlp.shared_expert.up_proj.weight")
            .unwrap();
        assert_eq!(d.semantic_role, TensorRole::Unknown);
    }

    #[test]
    fn an_ambiguous_qwen_alias_returns_candidates_not_a_silent_pick() {
        // NSIR-007. `Att` is ambiguous by design (ARCHITECTURE.md §6.2): it
        // could mean Q, K, V, or O, and the resolver says so instead of
        // choosing.
        let m = tiny_qwen_model();
        let r = m.resolve_alias("Att[10]").unwrap();
        assert!(r.is_ambiguous());
        assert_eq!(r.candidates.len(), 4);
        assert_eq!(r.confidence, 0.25);
        let roles: Vec<TensorRole> = r.candidates.iter().map(|c| c.role).collect();
        assert_eq!(
            roles,
            vec![
                TensorRole::AttentionQueryProjection,
                TensorRole::AttentionKeyProjection,
                TensorRole::AttentionValueProjection,
                TensorRole::AttentionOutputProjection,
            ]
        );
        match r.unique() {
            Err(QError::AmbiguousAlias { candidates, .. }) => {
                assert_eq!(candidates.len(), 4);
                assert!(candidates
                    .iter()
                    .any(|c| c == "model.layers[10].self_attention.query_projection.weight"));
            }
            other => panic!("expected AmbiguousAlias, got {other:?}"),
        }

        // `QNorm` covers Qwen3's two per-head norms, which Llama checkpoints do
        // not carry: also ambiguous, also candidates.
        let n = m.resolve_alias("QKNorm[10]").unwrap();
        assert!(n.is_ambiguous());
        assert_eq!(n.candidates.len(), 2);
        assert!(matches!(n.unique(), Err(QError::AmbiguousAlias { .. })));
    }

    // ------------------------------------------------------------------------
    // A characterization test — QM-0011.
    //
    // What follows RECORDS current behaviour. It does not endorse it, and it is
    // not a requirement. `.plan/PLAN_CHANGELOG.md` (2026-08-05, "the `experts.`
    // marker is unanchored") already carries the defect and the intended fix:
    // anchor the marker on a path-segment boundary. That fix is a **production**
    // change to `split_structure` above, which is byte-identical to base and
    // shared with the Llama resolver, so it is outside `QM-0011`'s test-only
    // boundary. The instruction that governs this case is explicit: if the fix
    // is out of scope, document the behaviour without blessing it.
    //
    // When the marker is anchored, this test must be REPLACED by one asserting
    // `expert == None` and `!resolved` — not deleted quietly, and not adjusted
    // to keep passing.
    // ------------------------------------------------------------------------

    #[test]
    fn an_unanchored_expert_marker_files_a_plural_shared_experts_name_as_routed_today() {
        // `"experts."` is a substring of `"shared_experts."`, and the search at
        // `split_structure`'s expert-marker branch is not anchored to a segment
        // boundary, so the plural indexed spelling is read as routed expert 3.
        let s = split_structure(
            "model.layers.0.mlp.shared_experts.3.up_proj.weight",
            "layers",
            Some("experts"),
        );
        assert_eq!(s.layer, Some(0));
        assert_eq!(
            s.expert,
            Some(3),
            "recorded, not endorsed: a shared expert read as a routed one"
        );
        assert_eq!(s.suffix, "up_proj.weight");

        // And end to end, through the public API, for both families that
        // declare an expert segment.
        let reg = Registry::builtin().unwrap();
        for id in ["llama", "qwen"] {
            let r = NsirResolver::new(reg.get(id).unwrap());
            let got = r.resolve_name("model.layers.0.mlp.shared_experts.3.up_proj.weight");
            assert!(got.resolved, "{id}");
            assert_eq!(got.expert, Some(3), "{id}");
            assert_eq!(
                canonical_name(&got).unwrap(),
                "model.layers[0].moe.experts[3].up_projection.weight",
                "{id}"
            );
        }

        // The two spellings this repository actually pins — Qwen2-MoE's
        // SINGULAR, unindexed `shared_expert.` and its gate — are unaffected and
        // stay `unknown`, which is why nothing shipped is wrong today. Whether
        // any real checkpoint emits the plural indexed spelling above is **not**
        // established by anything in this repository, and this test asserts
        // nothing either way about that.
        let r = NsirResolver::new(reg.get("qwen").unwrap());
        for raw in [
            "model.layers.0.mlp.shared_expert.up_proj.weight",
            "model.layers.0.mlp.shared_expert_gate.weight",
        ] {
            let got = r.resolve_name(raw);
            assert!(!got.resolved, "{raw}");
            assert_eq!(got.expert, None, "{raw}");
        }
    }

    #[test]
    fn an_unambiguous_qwen_alias_resolves_to_one_canonical_address() {
        let m = tiny_qwen_model();
        let q = m.resolve_alias("Q[10]").unwrap();
        assert!(!q.is_ambiguous());
        assert_eq!(q.confidence, 1.0);
        assert_eq!(
            q.unique().unwrap().canonical_name,
            "model.layers[10].self_attention.query_projection.weight"
        );

        let e = m.resolve_alias("Expert[10,37].up").unwrap();
        assert_eq!(
            e.unique().unwrap().canonical_name,
            "model.layers[10].moe.experts[37].up_projection.weight"
        );

        let router = m.resolve_alias("Router[10]").unwrap();
        assert_eq!(
            router.unique().unwrap().canonical_name,
            "model.layers[10].router.expert_routing.weight"
        );

        // An alias the Qwen manifest does not declare is rejected with an
        // explanation, not answered with a guess.
        let err = m.resolve_alias("Zzz[10]").unwrap_err();
        assert!(err.to_string().contains("unknown alias"), "{err}");
    }
}
