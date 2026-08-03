//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §4.2).
//!
//! [`NsirRecord`] — the structured semantic record a resolver produces.
//!
//! ARCHITECTURE.md §4.2 specifies exactly this shape:
//!
//! ```json
//! { "stack": "language", "layer": 10, "component": "attention",
//!   "operation": "query_projection", "parameter": "weight",
//!   "axes": ["output_channel", "input_channel"] }
//! ```
//!
//! with the MoE variant adding `"expert": 37`.
//!
//! `resolved == false` is a first-class outcome, not a failure: it means the
//! resolver read the name and did not recognise it. Downstream code must
//! surface that, never paper over it.

use q_source::role::{Component, Stack, TensorRole};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsirRecord {
    pub stack: Stack,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<u32>,
    pub component: Component,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expert: Option<u32>,
    /// e.g. `query_projection`. Empty when unresolved.
    pub operation: String,
    /// e.g. `weight`, `bias`.
    pub parameter: String,
    /// Axis labels, in tensor axis order.
    pub axes: Vec<String>,
    pub role: TensorRole,
    /// Which plugin produced this record.
    pub resolver_id: String,
    /// `false` when the resolver did not recognise the name.
    pub resolved: bool,
}

impl NsirRecord {
    /// The record for a name no resolver recognised.
    ///
    /// Note what is *absent*: no role guessed from shape, no invented
    /// operation, no fabricated axis labels.
    pub fn unknown(resolver_id: impl Into<String>, layer: Option<u32>, parameter: &str) -> Self {
        Self {
            stack: Stack::Unknown,
            layer,
            component: Component::Unknown,
            expert: None,
            operation: String::new(),
            parameter: parameter.to_string(),
            axes: Vec::new(),
            role: TensorRole::Unknown,
            resolver_id: resolver_id.into(),
            resolved: false,
        }
    }

    pub fn is_moe(&self) -> bool {
        self.expert.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_record_asserts_nothing() {
        let r = NsirRecord::unknown("generic", Some(4), "weight");
        assert!(!r.resolved);
        assert_eq!(r.role, TensorRole::Unknown);
        assert_eq!(r.component, Component::Unknown);
        assert!(r.axes.is_empty());
        assert!(r.operation.is_empty());
        // Structure that *is* evident from the name is still recorded.
        assert_eq!(r.layer, Some(4));
        assert_eq!(r.parameter, "weight");
    }

    #[test]
    fn serialization_matches_the_architecture_md_shape() {
        let r = NsirRecord {
            stack: Stack::Language,
            layer: Some(10),
            component: Component::Attention,
            expert: None,
            operation: "query_projection".into(),
            parameter: "weight".into(),
            axes: vec!["output_channel".into(), "input_channel".into()],
            role: TensorRole::AttentionQueryProjection,
            resolver_id: "llama".into(),
            resolved: true,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["layer"], 10);
        assert_eq!(v["operation"], "query_projection");
        assert_eq!(v["parameter"], "weight");
        assert_eq!(v["axes"][0], "output_channel");
        // `expert` is omitted for non-MoE tensors.
        assert!(v.get("expert").is_none());
    }

    #[test]
    fn moe_records_carry_the_expert_index() {
        let r = NsirRecord {
            stack: Stack::Language,
            layer: Some(10),
            component: Component::MoE,
            expert: Some(37),
            operation: "down_projection".into(),
            parameter: "weight".into(),
            axes: vec![],
            role: TensorRole::MoeExpertDownProjection,
            resolver_id: "llama".into(),
            resolved: true,
        };
        assert!(r.is_moe());
        assert_eq!(serde_json::to_value(&r).unwrap()["expert"], 37);
    }
}
