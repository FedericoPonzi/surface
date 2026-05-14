//! Scenario declarations (§8).

use crate::expr::{CallArg, Expr};
use crate::ident::{Ident, QualifiedName};
use crate::span::Span;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Scenario {
    pub title: String,
    pub kind: ScenarioKind,
    pub tags: Vec<Ident>,
    pub actors: Vec<ScenarioActor>,
    pub clauses: Vec<ScenarioClause>,
    pub requires_in: Vec<Ident>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ScenarioKind {
    Safety,
    Liveness,
    Forbidden,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ScenarioActor {
    pub name: Ident,
    pub actor_ty: Ident,
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ScenarioClause {
    Given { predicate: Expr, span: Span },
    /// `<Actor> does <Action>(args)`
    When { actor: Ident, action: QualifiedName, args: Vec<CallArg>, span: Span },
    /// `when atomic { <steps> }` — a sequence of `does` calls.
    WhenAtomic { steps: Vec<ScenarioClause>, span: Span },
    Then { predicate: Expr, span: Span },
    /// `then fails with <ErrName>`
    ThenFailsWith { error_name: Ident, span: Span },
    /// `observed E(args) by <Actor>`
    Observed { event: Ident, args: Vec<CallArg>, by_actor: Ident, span: Span },
}
