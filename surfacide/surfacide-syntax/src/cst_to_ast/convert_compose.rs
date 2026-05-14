//! Convert compose blocks (members, channels, realizes, acknowledged,
//! fairness).

use super::convert_expr::convert_expr;
use super::convert_substrate::{convert_channel_decl, convert_realizes_block};
use super::{
    convert_fairness_decl, doc_string_text, ident_from_node, Cvt,
};
use surfacide_ast::compose::*;
use surfacide_ast::expr::{Expr, ExprKind, PathAccessor};
use surfacide_ast::*;
use tree_sitter::Node;

pub fn convert_compose_block(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<ComposeBlock> {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("name")
        .map(|n| ident_from_node(n, cvt))?;
    let mut members: Vec<Ident> = Vec::new();
    let mut cursor = node.walk();
    for c in node.children_by_field_name("part", &mut cursor) {
        if c.kind() == "identifier" {
            members.push(ident_from_node(c, cvt));
        }
    }

    let mut channels = Vec::new();
    let mut realizes = None;
    let mut acknowledged: Option<AcknowledgedBlock> = None;
    let mut fairness = Vec::new();
    for child in cvt.all_named_children(node) {
        match child.kind() {
            "doc_string" => {}
            "channel_decl" => {
                if let Some(c) = convert_channel_decl(child, cvt) {
                    channels.push(c);
                }
            }
            "realizes_block" => {
                realizes = Some(convert_realizes_block(child, cvt));
            }
            "acknowledged_block" => {
                acknowledged = Some(convert_acknowledged_block(child, cvt));
            }
            "fairness_decl" => {
                if let Some(f) = convert_fairness_decl(child, cvt) {
                    fairness.push(f);
                }
            }
            // `epoch_decl` may appear in compose grammar but the typed
            // ComposeBlock does not yet model it; epoch handling at the
            // compose level is M2 work.
            _ => {}
        }
    }

    Some(ComposeBlock {
        name,
        members,
        channels,
        realizes,
        acknowledged,
        fairness,
        doc,
        span,
    })
}

// =============================================================================
// acknowledged { … }
// =============================================================================

pub(super) fn convert_acknowledged_block(node: Node, cvt: &mut Cvt) -> AcknowledgedBlock {
    let span = cvt.span(node);
    let entries: Vec<AcknowledgedEntry> = cvt
        .named_children_of(node, &["ack_entry"])
        .into_iter()
        .map(|e| convert_ack_entry(e, cvt))
        .collect();
    AcknowledgedBlock { entries, span }
}

fn convert_ack_entry(node: Node, cvt: &mut Cvt) -> AcknowledgedEntry {
    let span = cvt.span(node);
    let kind_node = node.child_by_field_name("kind");
    let kind_name = kind_node.map(|n| cvt.text(n)).unwrap_or("");
    let kind = match ObligationKind::from_name(kind_name) {
        Some(k) => k,
        None => {
            // Self-review should-fix #1: don't silently coerce typos to
            // `availability_depends_on` — emit an error pinned at the
            // kind token so the reviewer sees their typo, and default
            // the parsed kind to something that won't accidentally
            // discharge real rules.
            cvt.diags.push(
                surfacide_diag::Diagnostic::error(
                    surfacide_diag::ErrorKind::SurfaceSlotUnknownValue,
                    format!(
                        "unknown acknowledgement kind `{}`; expected one of: {}",
                        kind_name,
                        "availability_depends_on, availability_consistency, \
                         availability_channel_class, trust_transitive, information_flow, \
                         pii_anon, write_conflict, replay_amplification, \
                         retention_propagation, actor_view_leak, derived_write, \
                         freshness_channel"
                    ),
                    kind_node.map(|n| cvt.span(n)).unwrap_or(span),
                ),
            );
            // Use a sentinel that won't match any rule — InformationFlow
            // is not derived by any v0.10 rule, so it acts as inert.
            ObligationKind::InformationFlow
        }
    };

    let body = cvt.all_named_children(node).into_iter().find(|c| {
        matches!(c.kind(), "ack_list" | "ack_map")
    });

    let mut args: Vec<AcknowledgedArg> = Vec::new();
    let mut resolution: Option<AcknowledgedResolution> = None;
    let mut because: Option<String> = None;
    let mut because_span: Option<Span> = None;

    if let Some(b) = body {
        match b.kind() {
            "ack_list" => {
                for item in cvt.named_children_of(b, &["ack_list_item"]) {
                    for c in cvt.all_named_children(item) {
                        if c.kind() == "ack_because" {
                            if let Some((s, sp)) = extract_because(c, cvt) {
                                because = Some(s);
                                because_span = Some(sp);
                            }
                        } else {
                            args.push(arg_from_expr_node(c, cvt));
                        }
                    }
                }
            }
            "ack_map" => {
                for item in cvt.named_children_of(b, &["ack_map_item"]) {
                    let value_node = item.child_by_field_name("value");
                    for c in cvt.all_named_children(item) {
                        if Some(c) == value_node {
                            // The value side of `key : value` — treat as
                            // an expression-arg or a resolution if it looks
                            // like a call.
                            if let Some(r) = resolution_from_value(c, cvt) {
                                resolution = Some(r);
                            } else {
                                args.push(arg_from_expr_node(c, cvt));
                            }
                        } else if c.kind() == "ack_because" {
                            if let Some((s, sp)) = extract_because(c, cvt) {
                                because = Some(s);
                                because_span = Some(sp);
                            }
                        } else {
                            args.push(arg_from_expr_node(c, cvt));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    AcknowledgedEntry { kind, args, resolution, because, because_span, span }
}

fn extract_because(node: Node, cvt: &Cvt) -> Option<(String, Span)> {
    let s = node.child_by_field_name("reason")?;
    let raw = cvt.text(s);
    let stripped = raw
        .strip_prefix("\"\"\"")
        .and_then(|r| r.strip_suffix("\"\"\""))
        .or_else(|| raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')))
        .unwrap_or(raw);
    Some((stripped.to_string(), cvt.span(node)))
}

fn arg_from_expr_node(node: Node, cvt: &mut Cvt) -> AcknowledgedArg {
    if node.kind() == "identifier" {
        let id = ident_from_node(node, cvt);
        return AcknowledgedArg::Component(QualifiedName::new(vec![id]));
    }
    let e = convert_expr(node, cvt);
    match &*e.kind {
        ExprKind::Ident(i) => AcknowledgedArg::Component(QualifiedName::new(vec![i.clone()])),
        ExprKind::Path(p) if p.accessors.is_empty() => {
            AcknowledgedArg::Component(QualifiedName::new(vec![p.head.clone()]))
        }
        ExprKind::Path(p) => {
            // Convert path's field accessors back to a QualifiedName when
            // they are all field-access (no index).
            let mut segs = vec![p.head.clone()];
            for a in &p.accessors {
                if let PathAccessor::Field(f) = a {
                    segs.push(f.clone());
                } else {
                    return AcknowledgedArg::Other(e);
                }
            }
            AcknowledgedArg::Component(QualifiedName::new(segs))
        }
        _ => AcknowledgedArg::Other(e),
    }
}

fn resolution_from_value(node: Node, cvt: &mut Cvt) -> Option<AcknowledgedResolution> {
    if node.kind() != "call_expr" {
        return None;
    }
    let callee = node.child_by_field_name("callee")?;
    let name = match callee.kind() {
        "identifier" => cvt.text(callee).to_string(),
        "identifier_expr" => cvt
            .first_child(callee, "identifier")
            .map(|n| cvt.text(n).to_string())
            .unwrap_or_default(),
        _ => return None,
    };
    let args_node = cvt.first_child(node, "call_args")?;
    let arg_exprs: Vec<Expr> = cvt
        .all_named_children(args_node)
        .into_iter()
        .map(|a| {
            a.child_by_field_name("value")
                .map(|v| convert_expr(v, cvt))
                .unwrap_or(Expr {
                    kind: Box::new(ExprKind::LitNone),
                    span: cvt.span(a),
                })
        })
        .collect();
    let span = cvt.span(node);
    Some(AcknowledgedResolution::Other(
        Ident::new(name, span),
        arg_exprs,
    ))
}
