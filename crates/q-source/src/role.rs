//! Data plane: **Metadata Plane** (ARCHITECTURE.md §2.1, §4.2).
//!
//! Semantic roles for tensors.
//!
//! ARCHITECTURE.md §4.2 is explicit: *"The resolver must be allowed to return
//! `unknown`. It must never guess a semantic role just because two tensors
//! share the same shape."* [`TensorRole::Unknown`] is therefore a first-class
//! value, not an error, and nothing in this crate infers a role from a shape.

use serde::{Deserialize, Serialize};

/// Which top-level stack a tensor belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Stack {
    Language,
    Vision,
    Audio,
    Unknown,
}

/// Which component within a layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Component {
    Embedding,
    Attention,
    Mlp,
    MoE,
    Normalization,
    Router,
    OutputHead,
    Unknown,
}

/// The role of a tensor in the computation.
///
/// `Unknown` means "this resolver does not know", and is the correct answer for
/// any name a resolver has not been taught. Consumers must handle it; nothing
/// downstream may substitute a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TensorRole {
    TokenEmbedding,
    PositionEmbedding,
    AttentionQueryProjection,
    AttentionKeyProjection,
    AttentionValueProjection,
    AttentionOutputProjection,
    AttentionQueryNorm,
    AttentionKeyNorm,
    MlpGateProjection,
    MlpUpProjection,
    MlpDownProjection,
    MoeRouter,
    MoeExpertGateProjection,
    MoeExpertUpProjection,
    MoeExpertDownProjection,
    InputLayerNorm,
    PostAttentionLayerNorm,
    FinalNorm,
    LmHead,
    Bias,
    Unknown,
}

impl TensorRole {
    /// Stable snake_case name used in the catalog, the HTTP API, and WeightQL
    /// `WHERE role = "..."` filters.
    pub fn as_str(self) -> &'static str {
        match self {
            TensorRole::TokenEmbedding => "token_embedding",
            TensorRole::PositionEmbedding => "position_embedding",
            TensorRole::AttentionQueryProjection => "attention_query_projection",
            TensorRole::AttentionKeyProjection => "attention_key_projection",
            TensorRole::AttentionValueProjection => "attention_value_projection",
            TensorRole::AttentionOutputProjection => "attention_output_projection",
            TensorRole::AttentionQueryNorm => "attention_query_norm",
            TensorRole::AttentionKeyNorm => "attention_key_norm",
            TensorRole::MlpGateProjection => "mlp_gate_projection",
            TensorRole::MlpUpProjection => "mlp_up_projection",
            TensorRole::MlpDownProjection => "mlp_down_projection",
            TensorRole::MoeRouter => "moe_router",
            TensorRole::MoeExpertGateProjection => "moe_expert_gate_projection",
            TensorRole::MoeExpertUpProjection => "moe_expert_up_projection",
            TensorRole::MoeExpertDownProjection => "moe_expert_down_projection",
            TensorRole::InputLayerNorm => "input_layernorm",
            TensorRole::PostAttentionLayerNorm => "post_attention_layernorm",
            TensorRole::FinalNorm => "final_norm",
            TensorRole::LmHead => "lm_head",
            TensorRole::Bias => "bias",
            TensorRole::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> TensorRole {
        match s {
            "token_embedding" => TensorRole::TokenEmbedding,
            "position_embedding" => TensorRole::PositionEmbedding,
            "attention_query_projection" => TensorRole::AttentionQueryProjection,
            "attention_key_projection" => TensorRole::AttentionKeyProjection,
            "attention_value_projection" => TensorRole::AttentionValueProjection,
            "attention_output_projection" => TensorRole::AttentionOutputProjection,
            "attention_query_norm" => TensorRole::AttentionQueryNorm,
            "attention_key_norm" => TensorRole::AttentionKeyNorm,
            "mlp_gate_projection" => TensorRole::MlpGateProjection,
            "mlp_up_projection" => TensorRole::MlpUpProjection,
            "mlp_down_projection" => TensorRole::MlpDownProjection,
            "moe_router" => TensorRole::MoeRouter,
            "moe_expert_gate_projection" => TensorRole::MoeExpertGateProjection,
            "moe_expert_up_projection" => TensorRole::MoeExpertUpProjection,
            "moe_expert_down_projection" => TensorRole::MoeExpertDownProjection,
            "input_layernorm" => TensorRole::InputLayerNorm,
            "post_attention_layernorm" => TensorRole::PostAttentionLayerNorm,
            "final_norm" => TensorRole::FinalNorm,
            "lm_head" => TensorRole::LmHead,
            "bias" => TensorRole::Bias,
            _ => TensorRole::Unknown,
        }
    }

    pub fn component(self) -> Component {
        match self {
            TensorRole::TokenEmbedding | TensorRole::PositionEmbedding => Component::Embedding,
            TensorRole::AttentionQueryProjection
            | TensorRole::AttentionKeyProjection
            | TensorRole::AttentionValueProjection
            | TensorRole::AttentionOutputProjection
            | TensorRole::AttentionQueryNorm
            | TensorRole::AttentionKeyNorm => Component::Attention,
            TensorRole::MlpGateProjection
            | TensorRole::MlpUpProjection
            | TensorRole::MlpDownProjection => Component::Mlp,
            TensorRole::MoeExpertGateProjection
            | TensorRole::MoeExpertUpProjection
            | TensorRole::MoeExpertDownProjection => Component::MoE,
            TensorRole::MoeRouter => Component::Router,
            TensorRole::InputLayerNorm
            | TensorRole::PostAttentionLayerNorm
            | TensorRole::FinalNorm => Component::Normalization,
            TensorRole::LmHead => Component::OutputHead,
            TensorRole::Bias | TensorRole::Unknown => Component::Unknown,
        }
    }

    pub fn is_known(self) -> bool {
        self != TensorRole::Unknown
    }
}

impl Component {
    pub fn as_str(self) -> &'static str {
        match self {
            Component::Embedding => "embedding",
            Component::Attention => "attention",
            Component::Mlp => "mlp",
            Component::MoE => "moe",
            Component::Normalization => "normalization",
            Component::Router => "router",
            Component::OutputHead => "output_head",
            Component::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Component {
        match s {
            "embedding" => Component::Embedding,
            "attention" => Component::Attention,
            "mlp" => Component::Mlp,
            "moe" => Component::MoE,
            "normalization" => Component::Normalization,
            "router" => Component::Router,
            "output_head" => Component::OutputHead,
            _ => Component::Unknown,
        }
    }
}

impl Stack {
    pub fn as_str(self) -> &'static str {
        match self {
            Stack::Language => "language",
            Stack::Vision => "vision",
            Stack::Audio => "audio",
            Stack::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Stack {
        match s {
            "language" => Stack::Language,
            "vision" => Stack::Vision,
            "audio" => Stack::Audio,
            _ => Stack::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_strings_round_trip() {
        for r in [
            TensorRole::TokenEmbedding,
            TensorRole::AttentionQueryProjection,
            TensorRole::MoeExpertDownProjection,
            TensorRole::FinalNorm,
            TensorRole::Unknown,
        ] {
            assert_eq!(TensorRole::parse(r.as_str()), r);
        }
    }

    #[test]
    fn unrecognised_role_string_becomes_unknown_not_panic() {
        assert_eq!(TensorRole::parse("something_new"), TensorRole::Unknown);
        assert!(!TensorRole::Unknown.is_known());
    }

    #[test]
    fn components_are_derived_not_guessed() {
        assert_eq!(
            TensorRole::AttentionQueryProjection.component(),
            Component::Attention
        );
        assert_eq!(TensorRole::Unknown.component(), Component::Unknown);
    }
}
