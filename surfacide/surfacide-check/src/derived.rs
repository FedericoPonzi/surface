//! M3 — derived-field assignment static error (spec §6.6).
//!
//! A surface state field declared `derived` (with or without shape)
//! must not appear as the target of any `:= / += / -= / :+ / delete`
//! statement inside an action body. Emits `E_DERIVED_ASSIGN`.

use surfacide_ast::expr::{Expr, ExprKind, PathExpr};
use surfacide_ast::surface::{
    ActionDecl, EffectBlock, EffectStmt, StateFieldKind, SurfaceBlock,
};
use surfacide_ast::{Decl, Ident, Project};
use surfacide_diag::{Diagnostic, ErrorKind};
use std::collections::HashSet;

pub fn run(project: &Project) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (_key, module) in &project.modules {
        for decl in &module.decls {
            if let Decl::Surface(s) = decl {
                check_surface(s, &mut diagnostics);
            }
        }
    }
    diagnostics
}

fn check_surface(surface: &SurfaceBlock, out: &mut Vec<Diagnostic>) {
    let derived: HashSet<String> = surface
        .state
        .iter()
        .filter(|f| matches!(f.kind, StateFieldKind::Derived { .. }))
        .map(|f| f.name.name.clone())
        .collect();

    if derived.is_empty() {
        return;
    }

    // Also forbid `init { derived_field := … }`.
    for init in &surface.init {
        if derived.contains(&init.name.name) {
            out.push(diagnose(&init.name, &init.name.name));
        }
    }

    for a in &surface.actions {
        check_action(a, &derived, out);
    }
    for ia in &surface.internal_actions {
        check_action(&ia.action, &derived, out);
    }
}

fn check_action(a: &ActionDecl, derived: &HashSet<String>, out: &mut Vec<Diagnostic>) {
    walk_block(&a.body, derived, out);
}

fn walk_block(b: &EffectBlock, derived: &HashSet<String>, out: &mut Vec<Diagnostic>) {
    for stmt in &b.stmts {
        walk_stmt(stmt, derived, out);
    }
}

fn walk_stmt(s: &EffectStmt, derived: &HashSet<String>, out: &mut Vec<Diagnostic>) {
    match s {
        EffectStmt::Assign { target, .. }
        | EffectStmt::AddAssign { target, .. }
        | EffectStmt::SubAssign { target, .. }
        | EffectStmt::SeqSnoc { target, .. }
        | EffectStmt::DeleteKey { target, .. } => {
            if let Some(name) = lhs_head_name(target) {
                if derived.contains(&name.name) {
                    out.push(diagnose(name, &name.name));
                }
            }
        }
        EffectStmt::IfElse {
            then_block, else_block, ..
        } => {
            walk_block(then_block, derived, out);
            if let Some(eb) = else_block {
                walk_block(eb, derived, out);
            }
        }
        EffectStmt::For { body, .. } => walk_block(body, derived, out),
        EffectStmt::Match { arms, .. } => {
            for arm in arms {
                walk_block(&arm.body, derived, out);
            }
        }
        EffectStmt::IfLetSome {
            then_block,
            else_block,
            ..
        } => {
            walk_block(then_block, derived, out);
            if let Some(eb) = else_block {
                walk_block(eb, derived, out);
            }
        }
        _ => {}
    }
}

/// Return the head identifier of an assignment target expression.
fn lhs_head_name(e: &Expr) -> Option<&Ident> {
    match &*e.kind {
        ExprKind::Ident(id) => Some(id),
        ExprKind::Path(PathExpr { head, .. }) => Some(head),
        _ => None,
    }
}

fn diagnose(at: &Ident, name: &str) -> Diagnostic {
    Diagnostic::error(
        ErrorKind::DerivedAssign,
        format!(
            "cannot assign to surface state field `{}`: it is declared `derived` (spec §6.6)",
            name
        ),
        at.span,
    )
    .with_help(
        "derived fields are read-only on the surface side; the substrate's `maps` block \
         provides the projection",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use surfacide_ast::*;
    use surfacide_ast::surface::*;
    use surfacide_ast::expr::*;
    use surfacide_ast::decl::ActorBinder;

    fn id(s: &str) -> Ident {
        Ident::new(s, Span::synthetic())
    }

    fn derived_field(name: &str) -> StateField {
        StateField {
            name: id(name),
            ty: Type {
                kind: TypeKind::Nat,
                span: Span::synthetic(),
            },
            kind: StateFieldKind::Derived {
                shape: None,
                of_type: None,
            },
            retention: None,
            private: false,
            doc: None,
            span: Span::synthetic(),
        }
    }

    fn assign_to(name: &str) -> EffectStmt {
        EffectStmt::Assign {
            target: Expr {
                kind: Box::new(ExprKind::Ident(id(name))),
                span: Span::synthetic(),
            },
            value: Expr {
                kind: Box::new(ExprKind::LitNat(1)),
                span: Span::synthetic(),
            },
            span: Span::synthetic(),
        }
    }

    fn action_with(name: &str, stmts: Vec<EffectStmt>) -> ActionDecl {
        ActionDecl {
            name: id(name),
            params: Vec::new(),
            return_ty: None,
            actor: ActorBinder {
                name: id("u"),
                actor_ty: id("User"),
                span: Span::synthetic(),
            },
            when_pre: None,
            raises: Vec::new(),
            slots: Vec::new(),
            body: EffectBlock { stmts, span: Span::synthetic() },
            doc: None,
            span: Span::synthetic(),
            is_internal: false,
        }
    }

    fn surface_with(fields: Vec<StateField>, actions: Vec<ActionDecl>) -> SurfaceBlock {
        SurfaceBlock {
            state: fields,
            init: Vec::new(),
            fairness: Vec::new(),
            properties: Vec::new(),
            defaults: None,
            actions,
            internal_actions: Vec::new(),
            observables: Vec::new(),
            span: Span::synthetic(),
            doc: None,
        }
    }

    #[test]
    fn assign_to_derived_is_caught() {
        let s = surface_with(
            vec![derived_field("posts")],
            vec![action_with("post", vec![assign_to("posts")])],
        );
        let mut diags = Vec::new();
        check_surface(&s, &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_str(), "E_DERIVED_ASSIGN");
    }

    #[test]
    fn assign_to_plain_field_is_fine() {
        let s = surface_with(
            vec![],
            vec![action_with("post", vec![assign_to("next_seq")])],
        );
        let mut diags = Vec::new();
        check_surface(&s, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn nested_if_else_inside_derived_field_assignment_is_caught() {
        let inner = vec![assign_to("posts")];
        let if_stmt = EffectStmt::IfElse {
            cond: Expr {
                kind: Box::new(ExprKind::LitBool(true)),
                span: Span::synthetic(),
            },
            then_label: None,
            then_block: EffectBlock { stmts: inner, span: Span::synthetic() },
            else_label: None,
            else_block: None,
            span: Span::synthetic(),
        };
        let s = surface_with(
            vec![derived_field("posts")],
            vec![action_with("foo", vec![if_stmt])],
        );
        let mut diags = Vec::new();
        check_surface(&s, &mut diags);
        assert_eq!(diags.len(), 1);
    }
}
