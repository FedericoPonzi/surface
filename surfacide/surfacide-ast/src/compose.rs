//! Compose block and acknowledged{} obligation declarations.

use crate::expr::Expr;
use crate::ident::{Ident, QualifiedName};
use crate::span::Span;
use crate::substrate::{ChannelDecl, FairnessSpec, RealizesBlock};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ComposeBlock {
    pub name: Ident,
    pub members: Vec<Ident>,
    pub channels: Vec<ChannelDecl>,
    pub realizes: Option<RealizesBlock>,
    pub acknowledged: Option<AcknowledgedBlock>,
    pub fairness: Vec<FairnessSpec>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AcknowledgedBlock {
    pub entries: Vec<AcknowledgedEntry>,
    pub span: Span,
}

/// One acknowledgement entry. `because:` is per-entry (v0.10.1).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AcknowledgedEntry {
    pub kind: ObligationKind,
    pub args: Vec<AcknowledgedArg>,
    pub resolution: Option<AcknowledgedResolution>,
    pub because: Option<String>,
    pub because_span: Option<Span>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AcknowledgedArg {
    Component(QualifiedName),
    Field(Ident),
    Action(Ident),
    Other(Expr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ObligationKind {
    AvailabilityDependsOn,
    AvailabilityConsistency,
    AvailabilityChannelClass,
    TrustTransitive,
    InformationFlow,
    PiiAnon,
    WriteConflict,
    ReplayAmplification,
    RetentionPropagation,
    ActorViewLeak,
    DerivedWrite,
    FreshnessChannel,
}

impl ObligationKind {
    pub fn name(self) -> &'static str {
        match self {
            ObligationKind::AvailabilityDependsOn => "availability_depends_on",
            ObligationKind::AvailabilityConsistency => "availability_consistency",
            ObligationKind::AvailabilityChannelClass => "availability_channel_class",
            ObligationKind::TrustTransitive => "trust_transitive",
            ObligationKind::InformationFlow => "information_flow",
            ObligationKind::PiiAnon => "pii_anon",
            ObligationKind::WriteConflict => "write_conflict",
            ObligationKind::ReplayAmplification => "replay_amplification",
            ObligationKind::RetentionPropagation => "retention_propagation",
            ObligationKind::ActorViewLeak => "actor_view_leak",
            ObligationKind::DerivedWrite => "derived_write",
            ObligationKind::FreshnessChannel => "freshness_channel",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "availability_depends_on" => Self::AvailabilityDependsOn,
            "availability_consistency" => Self::AvailabilityConsistency,
            "availability_channel_class" => Self::AvailabilityChannelClass,
            "trust_transitive" => Self::TrustTransitive,
            "information_flow" => Self::InformationFlow,
            "pii_anon" => Self::PiiAnon,
            "write_conflict" => Self::WriteConflict,
            "replay_amplification" => Self::ReplayAmplification,
            "retention_propagation" => Self::RetentionPropagation,
            "actor_view_leak" => Self::ActorViewLeak,
            "derived_write" => Self::DerivedWrite,
            "freshness_channel" => Self::FreshnessChannel,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AcknowledgedResolution {
    /// `write_conflict` resolutions
    WriteConflict(WriteConflictResolution),
    /// `replay_amplification` resolutions
    Idempotent,
    DedupeKey(Vec<Ident>),
    IdempotentViaState,
    /// Generic: bare identifier (e.g. `serialized_by(...)`).
    Other(Ident, Vec<Expr>),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum WriteConflictResolution {
    SerializedBy(QualifiedName),
    LastWriterWins,
    Crdt(Ident),
    ForbiddenConcurrent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obligation_kind_round_trip() {
        for k in &[
            ObligationKind::AvailabilityDependsOn,
            ObligationKind::TrustTransitive,
            ObligationKind::WriteConflict,
            ObligationKind::ReplayAmplification,
            ObligationKind::RetentionPropagation,
            ObligationKind::ActorViewLeak,
            ObligationKind::DerivedWrite,
            ObligationKind::FreshnessChannel,
        ] {
            assert_eq!(ObligationKind::from_name(k.name()), Some(*k));
        }
    }
}
