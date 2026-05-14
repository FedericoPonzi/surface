//! M3 — cross-slot consistency checks.
//!
//! - `E_FRESHNESS_UNDECLARED_EPOCH` (§7.2.4): a freshness slot naming
//!   an epoch that no substrate declares.
//! - `E_SECRET_FLOW` (§6.5): `emit E(field=<expr>)` where `<expr>`
//!   reads a state field annotated `retention: secret`.
//! - `E_ACTOR_VIEW_LEAK` (§5.3): an observable whose parameter list
//!   carries an actor-typed param other than the `for u:` binder.
//!
//! These three checks reuse the surface-side AST + module-graph and
//! do not need name resolution beyond simple ident lookups within a
//! single surface.

use std::collections::HashSet;

use surfacide_ast::expr::{Expr, PathExpr};
use surfacide_ast::slot::{FreshnessValue, SlotKind, SlotValue};
use surfacide_ast::surface::{
    ActionDecl, EffectBlock, EffectStmt, RetentionClass, SurfaceBlock,
};
use surfacide_ast::ty::TypeKind;
use surfacide_ast::{Decl, Ident, Project};
use surfacide_diag::{Diagnostic, ErrorKind};

pub fn run(project: &Project) -> Vec<Diagnostic> {
    let actor_types = collect_actor_types(project);
    let declared_epochs = collect_declared_epochs(project);

    let mut diagnostics = Vec::new();
    for (_key, module) in &project.modules {
        for decl in &module.decls {
            if let Decl::Surface(s) = decl {
                check_secret_flow(s, &mut diagnostics);
                check_actor_view_leak(s, &actor_types, &mut diagnostics);
                check_freshness_epochs(s, &declared_epochs, &mut diagnostics);
            }
        }
    }
    diagnostics
}

fn collect_actor_types(project: &Project) -> HashSet<String> {
    let mut actors = HashSet::new();
    for (_key, module) in &project.modules {
        for decl in &module.decls {
            if let Decl::Actor(a) = decl {
                actors.insert(a.name.name.clone());
            }
        }
    }
    actors
}

fn collect_declared_epochs(project: &Project) -> HashSet<String> {
    let mut epochs = HashSet::new();
    for (_key, module) in &project.modules {
        for decl in &module.decls {
            match decl {
                Decl::Substrate(s) => {
                    for e in &s.epochs {
                        epochs.insert(e.name.name.clone());
                    }
                }
                Decl::PartialSubstrate(p) => {
                    for e in &p.block.epochs {
                        epochs.insert(e.name.name.clone());
                    }
                }
                _ => {}
            }
        }
    }
    epochs
}

// ── E_SECRET_FLOW ──

fn check_secret_flow(surface: &SurfaceBlock, out: &mut Vec<Diagnostic>) {
    let secret_fields: HashSet<String> = surface
        .state
        .iter()
        .filter(|f| matches!(f.retention, Some(RetentionClass::Secret)))
        .map(|f| f.name.name.clone())
        .collect();
    if secret_fields.is_empty() {
        return;
    }
    for a in &surface.actions {
        walk_block_emit(&a.body, &a.name, &secret_fields, out);
    }
    for ia in &surface.internal_actions {
        walk_block_emit(&ia.action.body, &ia.action.name, &secret_fields, out);
    }
}

fn walk_block_emit(
    block: &EffectBlock,
    action_name: &Ident,
    secret: &HashSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    for stmt in &block.stmts {
        walk_stmt_emit(stmt, action_name, secret, out);
    }
}

fn walk_stmt_emit(
    s: &EffectStmt,
    action_name: &Ident,
    secret: &HashSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    match s {
        EffectStmt::Emit { event, args, .. } => {
            for arg in args {
                if let Some(source) = expr_reads_secret(&arg.value, secret) {
                    let arg_label = arg
                        .name
                        .as_ref()
                        .map(|i| i.name.clone())
                        .unwrap_or_else(|| "<positional>".to_string());
                    out.push(
                        Diagnostic::error(
                            ErrorKind::SecretFlow,
                            format!(
                                "action `{}` emits `{}({}=…)` reading secret state field `{}` (spec §6.5)",
                                action_name.name, event.name, arg_label, source
                            ),
                            arg.span,
                        )
                        .with_help(
                            "remove the field from the emit, project it through a non-secret \
                             derived field, or weaken the field's `retention: secret` annotation",
                        ),
                    );
                }
            }
        }
        EffectStmt::IfElse {
            then_block,
            else_block,
            ..
        } => {
            walk_block_emit(then_block, action_name, secret, out);
            if let Some(eb) = else_block {
                walk_block_emit(eb, action_name, secret, out);
            }
        }
        EffectStmt::IfLetSome {
            then_block,
            else_block,
            ..
        } => {
            walk_block_emit(then_block, action_name, secret, out);
            if let Some(eb) = else_block {
                walk_block_emit(eb, action_name, secret, out);
            }
        }
        EffectStmt::For { body, .. } => walk_block_emit(body, action_name, secret, out),
        EffectStmt::Match { arms, .. } => {
            for arm in arms {
                walk_block_emit(&arm.body, action_name, secret, out);
            }
        }
        _ => {}
    }
}

/// If `e` reads a secret state field anywhere, return the first
/// matching field name. We walk every `Ident` / `Path` head we can see
/// in the expression. This is a structural (not type-aware) check —
/// false positives on shadowed locals would need name resolution to
/// remove.
fn expr_reads_secret(e: &Expr, secret: &HashSet<String>) -> Option<String> {
    let mut found = None;
    visit_path_heads(e, &mut |name| {
        if found.is_none() && secret.contains(name) {
            found = Some(name.to_string());
        }
    });
    found
}

fn visit_path_heads(e: &Expr, f: &mut dyn FnMut(&str)) {
    // Exhaustive over `ExprKind` so adding a new variant forces an
    // update — silent `_ => {}` would otherwise mask secret-flow
    // false negatives (caught in self-review-3).
    use surfacide_ast::expr::ExprKind as E;
    match &*e.kind {
        E::LitNat(_) | E::LitInt(_) | E::LitBool(_) | E::LitString(_) | E::LitNone => {}
        E::Ident(id) => f(&id.name),
        E::Path(PathExpr { head, .. }) => f(&head.name),
        E::Some_(inner) | E::UnaryOp(_, inner) | E::Cardinality(inner) => {
            visit_path_heads(inner, f)
        }
        E::Tuple(items) | E::SetLit(items) | E::SeqLit(items) => {
            for x in items {
                visit_path_heads(x, f);
            }
        }
        E::Record(fields) => {
            for fld in fields {
                visit_path_heads(&fld.value, f);
            }
        }
        E::MapLit(pairs) => {
            for (k, v) in pairs {
                visit_path_heads(k, f);
                visit_path_heads(v, f);
            }
        }
        E::BinOp(_, l, r) | E::Cross(l, r) => {
            visit_path_heads(l, f);
            visit_path_heads(r, f);
        }
        E::IsTest(inner, _) => visit_path_heads(inner, f),
        E::SetComprehension { binders, predicate, body } => {
            for b in binders {
                visit_path_heads(&b.domain, f);
            }
            if let Some(p) = predicate {
                visit_path_heads(p, f);
            }
            visit_path_heads(body, f);
        }
        E::MapComprehension { binders, predicate, key, value } => {
            for b in binders {
                visit_path_heads(&b.domain, f);
            }
            if let Some(p) = predicate {
                visit_path_heads(p, f);
            }
            visit_path_heads(key, f);
            visit_path_heads(value, f);
        }
        E::Forall(b, body) | E::Exists(b, body) => {
            visit_path_heads(&b.domain, f);
            visit_path_heads(body, f);
        }
        E::ChooseTyped { predicate, .. } => visit_path_heads(predicate, f),
        E::ChooseBounded { domain, predicate, .. } => {
            visit_path_heads(domain, f);
            visit_path_heads(predicate, f);
        }
        E::Aggregate(agg) => {
            visit_path_heads(&agg.expr, f);
            if let Some(s) = &agg.scope {
                visit_path_heads(s, f);
            }
            if let Some(fb) = &agg.fallback {
                visit_path_heads(fb, f);
            }
        }
        E::IfThenElse { cond, then_branch, else_branch } => {
            visit_path_heads(cond, f);
            visit_path_heads(then_branch, f);
            visit_path_heads(else_branch, f);
        }
        E::Match { scrutinee, arms } => {
            visit_path_heads(scrutinee, f);
            for arm in arms {
                visit_path_heads(&arm.body, f);
            }
        }
        E::IfLetSome { source, then_branch, else_branch, .. } => {
            visit_path_heads(source, f);
            visit_path_heads(then_branch, f);
            visit_path_heads(else_branch, f);
        }
        E::Let { value, body, .. } => {
            visit_path_heads(value, f);
            visit_path_heads(body, f);
        }
        E::EventsBefore(x)
        | E::EventsAfter(x)
        | E::FirstUnbounded(x)
        | E::LastUnbounded(x)
        | E::CountUnbounded(x)
        | E::StateAt(x) => visit_path_heads(x, f),
        E::Between(a, b)
        | E::FirstBounded(a, b)
        | E::LastBounded(a, b)
        | E::CountBounded(a, b) => {
            visit_path_heads(a, f);
            visit_path_heads(b, f);
        }
        E::Call { callee, args } => {
            visit_path_heads(callee, f);
            for a in args {
                visit_path_heads(&a.value, f);
            }
        }
    }
}

// ── E_ACTOR_VIEW_LEAK ──

fn check_actor_view_leak(
    surface: &SurfaceBlock,
    actor_types: &HashSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    for obs in &surface.observables {
        for param in &obs.params {
            if type_is_actor(&param.ty.kind, actor_types) {
                out.push(
                    Diagnostic::error(
                        ErrorKind::ActorViewLeak,
                        format!(
                            "observable `{}` has actor-typed parameter `{}: {}` — \
                             actor params let callers spawn views from another actor's perspective \
                             (spec §5.3)",
                            obs.name.name,
                            param.name.name,
                            type_display(&param.ty.kind),
                        ),
                        param.span,
                    )
                    .with_help(
                        "drop the parameter, or retype it as a non-actor index that the \
                         observable's `for <u>:` binder can dereference",
                    ),
                );
            }
        }
    }
}

fn type_is_actor(t: &TypeKind, actor_types: &HashSet<String>) -> bool {
    match t {
        TypeKind::Named(qn) => qn
            .segments
            .last()
            .map(|s| actor_types.contains(&s.name))
            .unwrap_or(false),
        _ => false,
    }
}

fn type_display(t: &TypeKind) -> String {
    match t {
        TypeKind::Named(qn) => qn
            .segments
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
            .join("."),
        _ => "<complex>".to_string(),
    }
}

// ── E_FRESHNESS_UNDECLARED_EPOCH ──

fn check_freshness_epochs(
    surface: &SurfaceBlock,
    declared: &HashSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    for a in &surface.actions {
        check_action_freshness(a, declared, out);
    }
    for ia in &surface.internal_actions {
        check_action_freshness(&ia.action, declared, out);
    }
}

fn check_action_freshness(
    action: &ActionDecl,
    declared: &HashSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    for sa in &action.slots {
        if sa.kind != SlotKind::Freshness {
            continue;
        }
        let SlotValue::Freshness(fv) = &sa.value else {
            continue;
        };
        let epoch = match fv {
            FreshnessValue::Bounded { epoch, .. } => Some(epoch),
            FreshnessValue::Eventual { epoch: Some(e) } => Some(e),
            FreshnessValue::StaleWhileRevalidate { epoch, .. } => Some(epoch),
            _ => None,
        };
        if let Some(ep) = epoch {
            if !declared.contains(&ep.name) {
                out.push(
                    Diagnostic::error(
                        ErrorKind::FreshnessUndeclaredEpoch,
                        format!(
                            "action `{}` references epoch `{}` in `freshness:` but no \
                             substrate declares it (spec §7.2.4)",
                            action.name.name, ep.name
                        ),
                        ep.span,
                    )
                    .with_help(
                        "add `epoch <name> { advances_on <action-set> ; covers <field-set> }` \
                         in the realising substrate, or use `freshness: strong` / `not_applicable`",
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use surfacide_ast::decl::{ActorBinder, ActorDecl, ObservableDecl, Param};
    use surfacide_ast::expr::{CallArg, Expr, ExprKind};
    use surfacide_ast::slot::{FreshnessValue, SlotAssign, SlotKind, SlotValue};
    use surfacide_ast::span::Span;
    use surfacide_ast::surface::{
        ActionDecl, EffectBlock, EffectStmt, RetentionClass, StateField, StateFieldKind,
        SurfaceBlock,
    };
    use surfacide_ast::ty::{Type, TypeKind};
    use surfacide_ast::QualifiedName;
    use surfacide_ast::Ident;

    fn id(s: &str) -> Ident {
        Ident::new(s, Span::synthetic())
    }

    fn bool_type() -> Type {
        Type { kind: TypeKind::Bool, span: Span::synthetic() }
    }

    fn named_type(name: &str) -> Type {
        Type {
            kind: TypeKind::Named(QualifiedName::new(vec![id(name)])),
            span: Span::synthetic(),
        }
    }

    fn empty_action(name: &str) -> ActionDecl {
        ActionDecl {
            name: id(name),
            params: Vec::new(),
            return_ty: None,
            actor: ActorBinder { name: id("u"), actor_ty: id("User"), span: Span::synthetic() },
            when_pre: None,
            raises: Vec::new(),
            slots: Vec::new(),
            body: EffectBlock { stmts: Vec::new(), span: Span::synthetic() },
            doc: None,
            span: Span::synthetic(),
            is_internal: false,
        }
    }

    fn ident_expr(name: &str) -> Expr {
        Expr {
            kind: Box::new(ExprKind::Ident(id(name))),
            span: Span::synthetic(),
        }
    }

    fn surface_with(state: Vec<StateField>, actions: Vec<ActionDecl>) -> SurfaceBlock {
        SurfaceBlock {
            state,
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

    fn secret_field(name: &str) -> StateField {
        StateField {
            name: id(name),
            ty: bool_type(),
            kind: StateFieldKind::Plain,
            retention: Some(RetentionClass::Secret),
            private: false,
            doc: None,
            span: Span::synthetic(),
        }
    }

    #[test]
    fn emit_of_secret_field_caught() {
        let mut a = empty_action("login");
        a.body.stmts.push(EffectStmt::Emit {
            event: id("LoggedIn"),
            args: vec![CallArg {
                name: Some(id("key")),
                value: ident_expr("signing_key"),
                span: Span::synthetic(),
            }],
            span: Span::synthetic(),
        });
        let s = surface_with(vec![secret_field("signing_key")], vec![a]);
        let mut diags = Vec::new();
        check_secret_flow(&s, &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_str(), "E_SECRET_FLOW");
    }

    #[test]
    fn emit_of_non_secret_field_is_fine() {
        let mut a = empty_action("post");
        a.body.stmts.push(EffectStmt::Emit {
            event: id("Posted"),
            args: vec![CallArg {
                name: Some(id("count")),
                value: ident_expr("post_count"),
                span: Span::synthetic(),
            }],
            span: Span::synthetic(),
        });
        let s = surface_with(vec![secret_field("signing_key")], vec![a]);
        let mut diags = Vec::new();
        check_secret_flow(&s, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn observable_with_actor_param_caught() {
        let obs = ObservableDecl {
            name: id("can_see_other"),
            for_actor: Some(ActorBinder {
                name: id("v"),
                actor_ty: id("Viewer"),
                span: Span::synthetic(),
            }),
            params: vec![Param {
                name: id("attacker"),
                ty: named_type("Viewer"),
                span: Span::synthetic(),
            }],
            return_ty: bool_type(),
            body: Expr {
                kind: Box::new(ExprKind::LitBool(true)),
                span: Span::synthetic(),
            },
            doc: None,
            span: Span::synthetic(),
        };
        let mut s = surface_with(Vec::new(), Vec::new());
        s.observables.push(obs);
        let actors: HashSet<String> = ["Viewer".to_string()].into_iter().collect();
        let mut diags = Vec::new();
        check_actor_view_leak(&s, &actors, &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_str(), "E_ACTOR_VIEW_LEAK");
    }

    #[test]
    fn observable_with_non_actor_index_param_is_fine() {
        let obs = ObservableDecl {
            name: id("at"),
            for_actor: Some(ActorBinder {
                name: id("v"),
                actor_ty: id("Viewer"),
                span: Span::synthetic(),
            }),
            params: vec![Param {
                name: id("i"),
                ty: bool_type(),
                span: Span::synthetic(),
            }],
            return_ty: bool_type(),
            body: Expr {
                kind: Box::new(ExprKind::LitBool(true)),
                span: Span::synthetic(),
            },
            doc: None,
            span: Span::synthetic(),
        };
        let mut s = surface_with(Vec::new(), Vec::new());
        s.observables.push(obs);
        let actors: HashSet<String> = ["Viewer".to_string()].into_iter().collect();
        let mut diags = Vec::new();
        check_actor_view_leak(&s, &actors, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn freshness_referencing_undeclared_epoch_caught() {
        let mut a = empty_action("get");
        a.slots.push(SlotAssign {
            kind: SlotKind::Freshness,
            value: SlotValue::Freshness(FreshnessValue::Bounded {
                epoch: id("CacheTick"),
                n: 3,
            }),
            span: Span::synthetic(),
        });
        let declared: HashSet<String> = HashSet::new();
        let mut diags = Vec::new();
        check_action_freshness(&a, &declared, &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code.as_str(), "E_FRESHNESS_UNDECLARED_EPOCH");
    }

    #[test]
    fn freshness_referencing_declared_epoch_is_fine() {
        let mut a = empty_action("get");
        a.slots.push(SlotAssign {
            kind: SlotKind::Freshness,
            value: SlotValue::Freshness(FreshnessValue::Bounded {
                epoch: id("CacheTick"),
                n: 3,
            }),
            span: Span::synthetic(),
        });
        let declared: HashSet<String> = ["CacheTick".to_string()].into_iter().collect();
        let mut diags = Vec::new();
        check_action_freshness(&a, &declared, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    #[allow(dead_code)]
    fn unused_actor_decl_import_keeps_compile() {
        // Compile-only: ensures the ActorDecl import works in case
        // tests evolve. (Suppresses unused-import warning.)
        let _: Option<ActorDecl> = None;
    }
}
