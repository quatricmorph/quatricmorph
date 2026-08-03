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
        let s = split_structure(
            raw,
            &naming.layer_segment,
            naming.expert_segment.as_deref(),
        );

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
    pub fn resolve_canonical(&self, address: &str) -> Result<(&TensorDescriptor, Option<ElementSelector>)> {
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
        let r = llama_resolver(&reg)
            .resolve_name("model.layers.10.mlp.experts.37.down_proj.weight");
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
                ds.push(descriptor(
                    &format!("model.layers.{layer}.{suffix}"),
                    shape,
                ));
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
        let (d2, _) = m.resolve_canonical("visual.blocks.0.attn.qkv.weight").unwrap();
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
}
