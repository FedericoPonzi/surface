//! Surface block: state, init, fairness, properties, actions,
//! internal_actions, defaults.

use crate::expr::Expr;
use crate::ident::Ident;
use crate::slot::SlotAssign;
use crate::span::Span;
use crate::ty::Type;
use crate::decl::{ActorBinder, Param};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SurfaceBlock {
    pub state: Vec<StateField>,
    pub init: Vec<InitAssignment>,
    pub fairness: Vec<crate::substrate::FairnessSpec>,
    pub properties: Vec<Property>,
    pub defaults: Option<DefaultsBlock>,
    pub actions: Vec<ActionDecl>,
    pub internal_actions: Vec<InternalActionDecl>,
    pub observables: Vec<crate::decl::ObservableDecl>,
    pub span: Span,
    pub doc: Option<String>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StateField {
    pub name: Ident,
    pub ty: Type,
    pub kind: StateFieldKind,
    pub retention: Option<RetentionClass>,
    pub private: bool,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum StateFieldKind {
    /// Plain mutable state.
    Plain,
    /// `derived [shape: <Shape> [of: <Type>]]` (§6.6).
    /// The projection itself lives in each substrate's `maps` block.
    Derived {
        shape: Option<DerivedShape>,
        of_type: Option<Type>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DerivedShape {
    PerActor,
    Aggregate,
    Snapshot,
    Indexed,
}

impl DerivedShape {
    pub fn name(self) -> &'static str {
        match self {
            DerivedShape::PerActor => "per_actor",
            DerivedShape::Aggregate => "aggregate",
            DerivedShape::Snapshot => "snapshot",
            DerivedShape::Indexed => "indexed",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "per_actor" => DerivedShape::PerActor,
            "aggregate" => DerivedShape::Aggregate,
            "snapshot" => DerivedShape::Snapshot,
            "indexed" => DerivedShape::Indexed,
            _ => return None,
        })
    }
}

/// State-field retention class (§6.5). A subset of the action-slot
/// retention enum: `waived` is not legal at the state level.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RetentionClass {
    Ephemeral,
    Transactional,
    Audit { period: Ident },
    Pii { class: crate::slot::PiiClass, ttl: Ident },
    Secret,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InitAssignment {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Property {
    pub name: Ident,
    pub kind: PropertyKind,
    pub body: Expr,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PropertyKind {
    /// `always P`
    Safety,
    /// `eventually P`
    Liveness,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DefaultsBlock {
    pub slots: Vec<SlotAssign>,
    pub span: Span,
}

/// A surface-block action (§6.4).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ActionDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub return_ty: Option<Type>,
    pub actor: ActorBinder,
    pub when_pre: Option<Expr>,
    pub raises: Vec<RaisesClause>,
    pub slots: Vec<SlotAssign>,
    pub body: EffectBlock,
    pub doc: Option<String>,
    pub span: Span,
    pub is_internal: bool,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct InternalActionDecl {
    pub action: ActionDecl, // is_internal = true
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RaisesClause {
    pub name: Ident,
    pub guard: Expr,
    pub span: Span,
}

/// `then { … }` effect body. A sequence of statements; one TLA+ next-state
/// disjunct per leaf branch.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EffectBlock {
    pub stmts: Vec<EffectStmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum EffectStmt {
    /// `x := e`
    Assign { target: crate::expr::Expr, value: Expr, span: Span },
    /// `x += e`
    AddAssign { target: crate::expr::Expr, value: Expr, span: Span },
    /// `x -= e`
    SubAssign { target: crate::expr::Expr, value: Expr, span: Span },
    /// `delete m[k]`
    DeleteKey { target: crate::expr::Expr, span: Span },
    /// `seq :+ e`
    SeqSnoc { target: crate::expr::Expr, value: Expr, span: Span },
    /// `emit E(arg=val, …)`
    Emit { event: Ident, args: Vec<crate::expr::CallArg>, span: Span },
    /// `sends Msg(args) [to <chan-or-component>]` — substrate-side
    /// effect. Critical: this is **not** the same as `emit` — it
    /// produces a channel fact for §15.1.5, not a user-visible event.
    /// (self-review must-fix #4)
    Sends {
        message: Ident,
        args: Vec<crate::expr::CallArg>,
        /// `to <ChannelName>` (named channel) — may be inferred when
        /// only one channel exists between two components.
        to_channel: Option<Ident>,
        /// `to <Component>` — shorthand for "the unique channel from
        /// us to this component".
        to_component: Option<crate::QualifiedName>,
        /// `to <Component>[<idExpr>]` — one replicate instance, N×M form.
        to_instance: Option<Expr>,
        span: Span,
    },
    /// `return e`
    Return { value: Expr, span: Span },
    /// `let x := e` (binding in the local scope).
    Let { name: Ident, value: Expr, span: Span },
    /// `if g then [<label>?] block else [<label>?] block`
    IfElse {
        cond: Expr,
        then_label: Option<BranchLabel>,
        then_block: EffectBlock,
        else_label: Option<BranchLabel>,
        else_block: Option<EffectBlock>,
        span: Span,
    },
    /// `for x in s do <effect-block>`
    For { name: Ident, domain: Expr, body: EffectBlock, span: Span },
    /// `match e { Some(x) -> … ; None -> … }` (statement form).
    Match { scrutinee: Expr, arms: Vec<EffectMatchArm>, span: Span },
    /// `if let Some(x) := e then … else …` (statement form).
    IfLetSome {
        name: Ident,
        source: Expr,
        then_block: EffectBlock,
        else_block: Option<EffectBlock>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EffectMatchArm {
    pub pattern: crate::expr::MatchPattern,
    pub body: EffectBlock,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BranchLabel {
    pub name: Ident,
    pub span: Span,
}
