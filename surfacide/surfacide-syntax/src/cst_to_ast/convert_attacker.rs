//! Convert attacker declarations.

use super::convert_expr::convert_expr;
use super::{convert_type_expr, ident_from_node, Cvt};
use surfacide_ast::decl::{AttackerCapability, AttackerDecl, Param};
use surfacide_ast::expr::{Expr, ExprKind};
use surfacide_ast::*;
use tree_sitter::Node;

pub fn convert_attacker_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<AttackerDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;

    let mut controls: Option<Param> = None;
    let mut initial: Option<Expr> = None;
    let mut may: Option<AttackerCapability> = None;
    let mut goal: Option<Expr> = None;

    for child in cvt.all_named_children(node) {
        match child.kind() {
            "attacker_controls" => {
                let cspan = cvt.span(child);
                let var = child
                    .child_by_field_name("var")
                    .map(|n| ident_from_node(n, cvt))
                    .unwrap_or_else(|| Ident::new("<missing>", cspan));
                let ty_id = child
                    .child_by_field_name("type")
                    .map(|n| ident_from_node(n, cvt))
                    .unwrap_or_else(|| Ident::new("<missing>", cspan));
                let ty = Type {
                    kind: TypeKind::Named(QualifiedName::new(vec![ty_id])),
                    span: cspan,
                };
                controls = Some(Param { name: var, ty, span: cspan });
            }
            "attacker_initial" => {
                if let Some(p) = child.child_by_field_name("predicate") {
                    initial = Some(convert_expr(p, cvt));
                }
            }
            "attacker_may" => {
                if let Some(v) = child.child_by_field_name("var") {
                    may = Some(AttackerCapability::AnyAllowedFor(ident_from_node(v, cvt)));
                }
            }
            "attacker_goal" => {
                // Attacker goals can be complex (`eventually emits Ev(...)`).
                // We synthesise a single Expr by either picking the first
                // expression child or falling back to a string literal of
                // the raw text. Resolution of the structured form lands in
                // M2.
                let span = cvt.span(child);
                let inner_expr = cvt
                    .all_named_children(child)
                    .into_iter()
                    .find(|c| c.kind() != "call_arg")
                    .map(|c| convert_expr(c, cvt));
                goal = Some(inner_expr.unwrap_or(Expr {
                    kind: Box::new(ExprKind::LitString(cvt.text(child).to_string())),
                    span,
                }));
            }
            _ => {}
        }
    }

    let controls = controls.unwrap_or(Param {
        name: Ident::new("<missing>", span),
        ty: Type {
            kind: TypeKind::Named(QualifiedName::new(vec![Ident::new("<missing>", span)])),
            span,
        },
        span,
    });
    let initial = initial.unwrap_or(Expr {
        kind: Box::new(ExprKind::LitBool(true)),
        span,
    });
    let may = may.unwrap_or(AttackerCapability::AnyAllowedFor(Ident::new("<missing>", span)));
    let goal = goal.unwrap_or(Expr {
        kind: Box::new(ExprKind::LitBool(true)),
        span,
    });

    Some(AttackerDecl {
        name,
        controls,
        initial,
        may,
        goal,
        doc,
        span,
    })
}
