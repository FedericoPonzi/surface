//! Identifiers and qualified names.

use crate::span::Span;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An identifier with a span and the source spelling.
///
/// We don't intern (yet) — the spelling is owned per node. This keeps the
/// MVP simple; interning is a later optimisation if we find the AST grows.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

impl Ident {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self { name: name.into(), span }
    }

    /// Construct a synthetic identifier (no source position).
    pub fn synthetic(name: impl Into<String>) -> Self {
        Self { name: name.into(), span: Span::synthetic() }
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

/// A dotted qualified name like `Banking.Ledger`.
///
/// Always non-empty. The first segment is the "leftmost" component.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct QualifiedName {
    pub segments: Vec<Ident>,
    pub span: Span,
}

impl QualifiedName {
    /// Construct a qualified name from one or more segments.
    /// Panics if `segments` is empty.
    pub fn new(segments: Vec<Ident>) -> Self {
        assert!(!segments.is_empty(), "QualifiedName requires >=1 segment");
        let span = segments
            .first()
            .unwrap()
            .span
            .merge(segments.last().unwrap().span);
        Self { segments, span }
    }

    pub fn last(&self) -> &Ident {
        self.segments.last().unwrap()
    }

    pub fn is_simple(&self) -> bool {
        self.segments.len() == 1
    }

    pub fn dotted(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.dotted())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::FileId;

    fn ident(s: &str) -> Ident {
        Ident::new(s, Span::new(FileId(0), 0, s.len() as u32))
    }

    #[test]
    fn ident_display() {
        assert_eq!(ident("Banking").to_string(), "Banking");
    }

    #[test]
    fn qualified_dotted() {
        let qn = QualifiedName::new(vec![ident("Banking"), ident("Ledger")]);
        assert_eq!(qn.dotted(), "Banking.Ledger");
        assert!(!qn.is_simple());
        assert_eq!(qn.last().name, "Ledger");
    }

    #[test]
    fn qualified_simple() {
        let qn = QualifiedName::new(vec![ident("X")]);
        assert!(qn.is_simple());
        assert_eq!(qn.to_string(), "X");
    }

    #[test]
    #[should_panic(expected = "QualifiedName requires >=1 segment")]
    fn empty_qualified_panics() {
        let _ = QualifiedName::new(vec![]);
    }
}
