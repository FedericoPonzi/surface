//! Render Diagnostic → miette report.

use crate::Diagnostic;
use miette::{LabeledSpan, MietteDiagnostic, Severity};
use surfacide_ast::FileRegistry;

/// Convert a [`Diagnostic`] into a `MietteDiagnostic` suitable for
/// rendering with the `miette::fancy` reporter.
pub fn to_miette(diag: &Diagnostic, files: &FileRegistry) -> MietteDiagnostic {
    let severity = if diag.is_error() { Severity::Error } else { Severity::Warning };
    let mut report = MietteDiagnostic::new(diag.message.clone())
        .with_code(diag.code.as_str())
        .with_severity(severity);

    let mut labels = Vec::new();
    if let Some(file) = files.get(diag.primary_span.file) {
        let _ = file; // source attached via miette source separately
        let start = diag.primary_span.start as usize;
        let len = diag.primary_span.len() as usize;
        labels.push(LabeledSpan::new(Some("here".into()), start, len));
    }
    for l in &diag.labels {
        if l.span.file == diag.primary_span.file {
            let start = l.span.start as usize;
            let len = l.span.len() as usize;
            labels.push(LabeledSpan::new(Some(l.message.clone()), start, len));
        }
    }
    report = report.with_labels(labels);

    if let Some(h) = &diag.help {
        report = report.with_help(h.clone());
    }
    report
}

/// Pretty-print every diagnostic to stderr.
pub fn print_all(diags: &[Diagnostic], files: &FileRegistry) {
    for d in diags {
        let report = to_miette(d, files);
        let source = files
            .source(d.primary_span.file)
            .map(|s| s.to_string())
            .unwrap_or_default();
        let path = files
            .path(d.primary_span.file)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());

        let report = miette::Report::new(report).with_source_code(miette::NamedSource::new(path, source));
        eprintln!("{:?}", report);
    }
}

/// Summarise diagnostics. Returns `(errors, warnings)` counts.
pub fn summarise(diags: &[Diagnostic]) -> (usize, usize) {
    let mut e = 0;
    let mut w = 0;
    for d in diags {
        if d.is_error() {
            e += 1;
        } else {
            w += 1;
        }
    }
    (e, w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorKind, WarningKind};
    use surfacide_ast::{FileId, Span};

    #[test]
    fn summarise_counts_correctly() {
        let span = Span::new(FileId(0), 0, 1);
        let diags = vec![
            Diagnostic::error(ErrorKind::ParseError, "bad", span),
            Diagnostic::warning(WarningKind::AckNoRule, "stuff", span),
            Diagnostic::error(ErrorKind::NameNotFound, "missing", span),
        ];
        assert_eq!(summarise(&diags), (2, 1));
    }
}
