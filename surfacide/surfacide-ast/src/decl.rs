//! Top-level declarations: types, actors, events, externs, observables,
//! history predicates, attackers.

use crate::ident::{Ident, QualifiedName};
use crate::span::Span;
use crate::ty::Type;
use crate::expr::Expr;
use crate::surface::SurfaceBlock;
use crate::substrate::{SubstrateBlock, PartialSubstrateBlock};
use crate::compose::ComposeBlock;
use crate::scenario::Scenario;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A module: header + list of declarations. Multiple `.surf` files
/// declaring the same module are unioned by [`crate::project::Project`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ModuleFile {
    pub header: ModuleHeader,
    pub uses: Vec<UseDecl>,
    pub decls: Vec<Decl>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ModuleHeader {
    pub name: QualifiedName,
    pub private: bool,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UseDecl {
    pub module: QualifiedName,
    pub items: Vec<Ident>,
    pub span: Span,
}

/// Every top-level declaration variant. Each carries a span via its
/// concrete struct.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Decl {
    TypeAlias(TypeAliasDecl),
    Actor(ActorDecl),
    Event(EventDecl),
    Const(ConstDecl),
    Extern(ExternDecl),
    Observable(ObservableDecl),
    HistoryPredicate(HistoryPredicateDecl),
    Attacker(AttackerDecl),
    Property(crate::surface::Property),
    Scenario(Scenario),
    Surface(SurfaceBlock),
    Substrate(SubstrateBlock),
    PartialSubstrate(PartialSubstrateBlock),
    Compose(ComposeBlock),
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ModuleDecl {
    pub header: ModuleHeader,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TypeAliasDecl {
    pub name: Ident,
    pub ty: Type,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ActorDecl {
    pub name: Ident,
    pub extends: Option<Ident>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EventDecl {
    pub name: Ident,
    pub fields: Vec<EventField>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct EventField {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ConstDecl {
    pub name: Ident,
    pub ty: Type,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ExternDecl {
    pub name: Ident,
    pub ty: Type,
    pub doc: Option<String>,
    pub span: Span,
}

/// A regular observable, or an actor-relative `observable for u: <Actor>`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ObservableDecl {
    pub name: Ident,
    pub for_actor: Option<ActorBinder>,
    pub params: Vec<Param>,
    pub return_ty: Type,
    pub body: Expr,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ActorBinder {
    pub name: Ident,
    pub actor_ty: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Param {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HistoryPredicateDecl {
    pub name: Ident,
    pub params: Vec<Param>,
    pub body: Expr,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AttackerDecl {
    pub name: Ident,
    pub controls: Param,
    pub initial: Expr,
    pub may: AttackerCapability,
    pub goal: Expr,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AttackerCapability {
    /// `may any action allowed for <var>`
    AnyAllowedFor(Ident),
}
