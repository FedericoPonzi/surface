//! Surface type expressions (§3 of the spec).
//!
//! The MVP type system is structural and shape-only. We don't perform
//! deep unification — the slot pass and obligation pass care about
//! retention class, derived-ness, and a few specific shapes. Full
//! semantic type checking is a v0.2+ goal.

use crate::ident::{Ident, QualifiedName};
use crate::span::Span;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TypeKind {
    Nat,
    Int,
    Bool,
    String,
    Duration,
    /// A named type — could be an actor, an event, a type alias, or
    /// a built-in like `User`. Resolution decides which.
    Named(QualifiedName),
    Set(Box<Type>),
    Seq(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Optional(Box<Type>),
    Tuple(Vec<Type>),
    Record(Vec<RecordTypeField>),
    /// `A | B | C` tagged union, top-level.
    Union(Vec<Type>),
    /// `enum { Red, Green, Blue }`.
    Enum(Vec<Ident>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RecordTypeField {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::FileId;

    #[test]
    fn nested_type_shape() {
        let f = FileId(0);
        let ty = Type {
            kind: TypeKind::Map(
                Box::new(Type { kind: TypeKind::Nat, span: Span::new(f, 0, 3) }),
                Box::new(Type {
                    kind: TypeKind::Set(Box::new(Type {
                        kind: TypeKind::Bool,
                        span: Span::new(f, 0, 4),
                    })),
                    span: Span::new(f, 0, 4),
                }),
            ),
            span: Span::new(f, 0, 10),
        };
        assert!(matches!(ty.kind, TypeKind::Map(_, _)));
    }
}
