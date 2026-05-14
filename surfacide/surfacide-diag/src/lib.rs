//! Diagnostic types for Surfacide passes.
//!
//! Each error/warning code has a corresponding variant of [`Diagnostic`].
//! Diagnostics carry source spans and render via `miette` with code
//! frames and suggestions.

pub mod codes;
pub mod render;

pub use codes::{Code, ErrorKind, WarningKind};

use surfacide_ast::Span;

/// A reportable diagnostic. Either an error or a warning, each with a
/// stable code that R-round reviewers (and `trycmd` integration tests)
/// rely on.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: Code,
    pub message: String,
    pub primary_span: Span,
    pub labels: Vec<DiagnosticLabel>,
    pub help: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticLabel {
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn error(code: ErrorKind, message: impl Into<String>, primary: Span) -> Self {
        Self {
            code: Code::Error(code),
            message: message.into(),
            primary_span: primary,
            labels: Vec::new(),
            help: None,
        }
    }

    pub fn warning(code: WarningKind, message: impl Into<String>, primary: Span) -> Self {
        Self {
            code: Code::Warning(code),
            message: message.into(),
            primary_span: primary,
            labels: Vec::new(),
            help: None,
        }
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(DiagnosticLabel { span, message: message.into() });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn is_error(&self) -> bool {
        matches!(self.code, Code::Error(_))
    }
}
