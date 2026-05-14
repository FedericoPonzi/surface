//! Substrate block: components, replicate, channels, maps, realizes,
//! internal, authentication, auxiliary, fairness, epoch.

use crate::expr::Expr;
use crate::ident::{Ident, QualifiedName};
use crate::span::Span;
use crate::surface::{BranchLabel, EffectBlock};
use crate::ty::Type;
use crate::decl::Param;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SubstrateBlock {
    pub name: Ident,
    pub realizes_target: Option<QualifiedName>, // `realizes <SurfaceName>`
    pub components: Vec<Component>,
    pub replicates: Vec<ReplicateBlock>,
    pub channels: Vec<ChannelDecl>,
    pub auxiliary: Option<AuxiliaryBlock>,
    pub authentication: Option<AuthenticationBlock>,
    pub maps: Option<MapsBlock>,
    pub realizes: Option<RealizesBlock>,
    pub internal: Option<InternalBlock>,
    pub fairness: Vec<FairnessSpec>,
    pub epochs: Vec<EpochDecl>,
    pub acknowledged: Option<crate::compose::AcknowledgedBlock>,
    pub doc: Option<String>,
    pub span: Span,
}

impl SubstrateBlock {
    /// Convenience: every `acknowledged { … }` block reachable from this
    /// substrate. For now there's at most one (top-level); helper is a
    /// `Vec<&AcknowledgedBlock>` to keep the obligation-pass interface
    /// stable when we later allow per-component acks.
    pub fn acknowledged_blocks(&self) -> Vec<&crate::compose::AcknowledgedBlock> {
        self.acknowledged.iter().collect()
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PartialSubstrateBlock {
    pub block: SubstrateBlock,
    pub owns: Vec<Ident>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Component {
    pub name: Ident,
    pub state: Vec<crate::surface::StateField>,
    pub init: Vec<crate::surface::InitAssignment>,
    pub actions: Vec<ComponentAction>,
    pub receives: Vec<ReceivesHandler>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReplicateBlock {
    pub component: Component,
    pub id_param: Ident,
    pub id_ty: Type,
    pub id_domain: Expr,
}

/// A substrate component action — no slots; only `when`/`then`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ComponentAction {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_ty: Option<Type>,
    pub when_pre: Option<Expr>,
    pub raises: Vec<crate::surface::RaisesClause>,
    pub body: EffectBlock,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReceivesHandler {
    pub message: Ident,
    pub params: Vec<Param>,
    pub from_channel: Ident,
    pub when_pre: Option<Expr>,
    pub body: EffectBlock,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ChannelDecl {
    pub name: Ident,
    pub from_comp: QualifiedName,
    pub from_mult: ChannelMultiplicity,
    pub to_comp: QualifiedName,
    pub to_mult: ChannelMultiplicity,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChannelMultiplicity {
    One,
    /// `A[*]`
    Star,
    /// `A[i]` — same-id-set pairwise
    PairwiseId(Ident),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SendsStmt {
    pub message: Ident,
    pub args: Vec<crate::expr::CallArg>,
    pub to_channel: Option<Ident>,
    pub to_component: Option<QualifiedName>,
    pub to_instance: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AuxiliaryBlock {
    pub vars: Vec<AuxiliaryVar>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AuxiliaryVar {
    pub kind: AuxiliaryKind,
    pub name: Ident,
    pub ty: Type,
    pub init: Option<Expr>,
    pub cross_visible: bool,
    pub invariant: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AuxiliaryKind {
    History,
    Prophecy,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AuthenticationBlock {
    pub mappings: Vec<AuthenticationMapping>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AuthenticationMapping {
    pub surface_action: QualifiedName,
    pub rhs: AuthRhs,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AuthRhs {
    /// `Comp.field…` or `Comp[id].field…`
    Path(crate::expr::PathExpr),
    /// `param.<argname>`
    Param(Ident),
    /// `system`
    System,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MapsBlock {
    pub mappings: Vec<MapsEntry>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MapsEntry {
    pub field: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RealizesBlock {
    pub clauses: Vec<RealizesClause>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RealizesClause {
    pub surface_action: QualifiedName,
    pub args: Vec<Ident>,
    pub channel_selector: ChannelSelector,
    pub branch_label: Option<BranchLabel>,
    pub target: RealizesTarget,
    pub when_guard: Option<Expr>,
    /// `for some <id> in <ids>` explicit existential binders.
    pub for_some: Vec<(Ident, Expr)>,
    pub span: Span,
}

/// v0.10.1 channel selector: a specific channel, the `[*]` wildcard, or none.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChannelSelector {
    None,
    /// `surface.A(...)[anonymous]`
    Specific(Ident),
    /// v0.10.1 `[*]` channel-agnostic — covers every channel branch.
    Star { span: Span },
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RealizesTarget {
    /// `by Comp.action` (possibly with replicate `Comp[id].action`).
    Action {
        component: QualifiedName,
        replicate_id: Option<Ident>,
        action: Ident,
    },
    /// `by EXTERNAL`
    External,
    /// `by stutter` (compose-level only).
    Stutter,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InternalBlock {
    pub entries: Vec<InternalEntry>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum InternalEntry {
    /// `Comp.action`
    Action { component: QualifiedName, action: Ident, span: Span },
    /// `Comp.*` — glob.
    AllOfComponent { component: QualifiedName, span: Span },
    /// `Comp[*].action`
    ActionAllReplicas { component: QualifiedName, action: Ident, span: Span },
    /// `Comp[*].*`
    AllOfAllReplicas { component: QualifiedName, span: Span },
    /// `Comp[*].receives.MsgName` / `Comp.receives.MsgName`
    Receives { component: QualifiedName, all_replicas: bool, message: Ident, span: Span },
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FairnessSpec {
    pub strength: FairnessStrength,
    pub target: FairnessTarget,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FairnessStrength { Weak, Strong }

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FairnessTarget {
    /// `<action>` on the surface side, or `<Comp>.<action>` on the substrate side.
    Path(QualifiedName),
    /// `<Comp>[*].<action>`
    AllReplicas { component: QualifiedName, action: Ident },
    /// `<Comp>[<id>].<action>`
    SpecificReplica { component: QualifiedName, id: Ident, action: Ident },
    /// `<Comp>[*].receives.<Msg>`
    ReceivesAllReplicas { component: QualifiedName, message: Ident },
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EpochDecl {
    pub name: Ident,
    pub advances_on: Vec<FairnessTarget>, // reuse target syntax
    pub covers: Vec<Ident>,
    pub doc: Option<String>,
    pub span: Span,
}
