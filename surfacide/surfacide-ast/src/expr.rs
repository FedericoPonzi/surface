//! Surface expression grammar (§3.1 and §6.1).
//!
//! One expression sublanguage used in: `when` guards, `raises` guards,
//! `then` effect values, `property` bodies, `maps` expressions,
//! `observable` bodies, `history_predicate` bodies, scenario predicates,
//! attacker conditions.

use crate::ident::{Ident, QualifiedName};
use crate::span::Span;
use crate::ty::Type;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Expr {
    pub kind: Box<ExprKind>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ExprKind {
    // Literals
    LitNat(u64),
    LitInt(i64),
    LitBool(bool),
    LitString(String),
    LitNone,

    // Variables and paths
    Ident(Ident),
    /// A dotted path like `Edge[id].cache_valid[(d, p)]`.
    /// Stored structurally: a head plus a chain of accessors.
    Path(PathExpr),

    // Constructors
    Some_(Expr),
    Tuple(Vec<Expr>),
    Record(Vec<RecordFieldInit>),
    SetLit(Vec<Expr>),
    SeqLit(Vec<Expr>),
    MapLit(Vec<(Expr, Expr)>),

    // Operators
    BinOp(BinOp, Expr, Expr),
    UnaryOp(UnaryOp, Expr),
    /// `e is EventName`
    IsTest(Expr, Ident),
    /// `|x|` — cardinality
    Cardinality(Expr),

    // Comprehensions
    SetComprehension {
        binders: Vec<ComprehensionBinder>,
        predicate: Option<Expr>,
        body: Expr,
    },
    MapComprehension {
        binders: Vec<ComprehensionBinder>,
        predicate: Option<Expr>,
        key: Expr,
        value: Expr,
    },

    // Quantifiers
    Forall(Binding, Expr),
    Exists(Binding, Expr),

    // Choose / aggregate
    /// `choose x: T. P` (typed form).
    ChooseTyped { name: Ident, ty: Type, predicate: Expr },
    /// `choose x in s. P` (bounded form).
    ChooseBounded { name: Ident, domain: Expr, predicate: Expr },
    /// `aggregate Comp[id].expr [over <scope>] using <agg> [else <e>]`
    Aggregate(Box<AggregateExpr>),

    // Control flow
    IfThenElse { cond: Expr, then_branch: Expr, else_branch: Expr },
    Match { scrutinee: Expr, arms: Vec<MatchArm> },
    /// `if let Some(x) := e then … else …` value form.
    IfLetSome { name: Ident, source: Expr, then_branch: Expr, else_branch: Expr },
    Let { name: Ident, value: Expr, body: Expr },

    // Event-log helpers (§9.1.1, §9.1.2)
    EventsBefore(Expr),
    EventsAfter(Expr),
    Between(Expr, Expr),
    FirstUnbounded(Expr),                 // first(p)
    FirstBounded(Expr, Expr),             // first(seq, p)
    LastUnbounded(Expr),
    LastBounded(Expr, Expr),
    CountUnbounded(Expr),
    CountBounded(Expr, Expr),
    StateAt(Expr),

    // Cross
    Cross(Expr, Expr),

    // Function-style call (observable, history_predicate, type constructors, etc.)
    Call { callee: Expr, args: Vec<CallArg> },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PathExpr {
    pub head: Ident,
    pub accessors: Vec<PathAccessor>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PathAccessor {
    /// `.field` or `.0`.
    Field(Ident),
    /// `.0` numeric tuple index.
    TupleIndex(u32),
    /// `[expr]` index.
    Index(Expr),
    /// `[id]` replicate-instance selector — a bare identifier; resolution
    /// decides whether it's a variable bound to an id or an expression.
    /// We keep this as a separate variant for diagnostic precision.
    Replicate(Ident),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ComprehensionBinder {
    pub pattern: BinderPattern,
    pub domain: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BinderPattern {
    /// `x` or `x: T` (type optional in comprehensions).
    Name(Ident, Option<Type>),
    /// `(a, b)` destructure over a tuple.
    Tuple(Vec<Ident>),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Binding {
    pub name: Ident,
    pub domain: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AggregateExpr {
    pub component: QualifiedName,
    pub binder: Ident,
    pub expr: Expr,
    pub scope: Option<Expr>,
    pub aggregator: AggregatorKind,
    pub fallback: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AggregatorKind {
    Exists,
    Forall,
    Sum,
    Max,
    Min,
    UnionSet,
    /// `concat_seq(order_by f)`
    ConcatSeq { order_by: Ident },
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MatchPattern {
    Some_(Ident),
    None_,
    /// `_` wildcard.
    Wildcard,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RecordFieldInit {
    pub name: Ident,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct CallArg {
    /// Named arg (`field=value`) or positional.
    pub name: Option<Ident>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum BinOp {
    Eq, NotEq, Lt, LtEq, Gt, GtEq,
    Add, Sub, Mul, Div,
    And, Or, Implies,
    In, NotIn,
    Union, Intersect, Diff,
    SeqSnoc, // :+
    Subset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum UnaryOp {
    Not,
    Neg,
}
