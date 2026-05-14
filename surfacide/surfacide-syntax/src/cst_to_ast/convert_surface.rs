//! Convert surface block constructs (state, init, defaults, properties,
//! actions, internal_actions, fairness, observables) and the standalone
//! `property` declaration.
//!
//! Also hosts the shared effect-block / slot-assign / state-field /
//! action-decl helpers that substrate components reuse via
//! [`convert_component_action_decl`] and [`convert_state_field_node`].

use super::convert_expr::{convert_call_arg, convert_expr};
use super::{
    convert_param_list, convert_qualified_name, convert_type_expr, doc_string_text,
    ident_from_node, Cvt,
};
use surfacide_ast::decl::{ActorBinder, Param};
use surfacide_ast::expr::{CallArg, Expr, ExprKind, MatchPattern, PathAccessor, PathExpr};
use surfacide_ast::slot::*;
use surfacide_ast::substrate::FairnessSpec;
use surfacide_ast::surface::*;
use surfacide_ast::*;
use tree_sitter::Node;

pub fn convert_surface_block(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<SurfaceBlock> {
    let span = cvt.span(node);
    let mut state: Vec<StateField> = Vec::new();
    let mut init: Vec<InitAssignment> = Vec::new();
    let mut defaults: Option<DefaultsBlock> = None;
    let mut fairness: Vec<FairnessSpec> = Vec::new();
    let mut properties: Vec<Property> = Vec::new();
    let mut actions: Vec<ActionDecl> = Vec::new();
    let mut internal_actions: Vec<InternalActionDecl> = Vec::new();
    let mut observables: Vec<ObservableDecl> = Vec::new();
    let mut pending_doc: Option<String> = None;

    for child in cvt.all_named_children(node) {
        match child.kind() {
            "doc_string" => {
                pending_doc = Some(doc_string_text(cvt.text(child)));
                continue;
            }
            "state_block" => {
                let mut inner_doc: Option<String> = None;
                for f in cvt.all_named_children(child) {
                    match f.kind() {
                        "doc_string" => inner_doc = Some(doc_string_text(cvt.text(f))),
                        "state_field" => {
                            state.push(convert_state_field(f, cvt, inner_doc.take()));
                        }
                        _ => {}
                    }
                }
            }
            "init_block" => {
                for f in cvt.all_named_children(child) {
                    if let Some(a) = convert_init_assignment(f, cvt) {
                        init.push(a);
                    }
                }
            }
            "defaults_block" => {
                let dspan = cvt.span(child);
                let slots: Vec<SlotAssign> = cvt
                    .named_children_of(child, &["slot_assign"])
                    .into_iter()
                    .map(|s| convert_slot_assign(s, cvt))
                    .collect();
                defaults = Some(DefaultsBlock { slots, span: dspan });
            }
            "fairness_decl" => {
                if let Some(f) = super::convert_fairness_decl(child, cvt) {
                    fairness.push(f);
                }
            }
            "property_decl" => {
                if let Some(p) = convert_property_decl(child, cvt, pending_doc.take()) {
                    properties.push(p);
                }
            }
            "action_decl" => {
                if let Some(a) = convert_action_decl(child, cvt, /*is_internal=*/ false, pending_doc.take()) {
                    actions.push(a);
                }
            }
            "internal_action_decl" => {
                if let Some(a) = convert_action_decl(child, cvt, /*is_internal=*/ true, pending_doc.take()) {
                    internal_actions.push(InternalActionDecl { action: a });
                }
            }
            "observable_decl" => {
                if let Some(o) = super::convert_observable_decl_pub(child, cvt, pending_doc.take()) {
                    observables.push(o);
                }
            }
            "actor_observable_decl" => {
                if let Some(o) = super::convert_actor_observable_decl_pub(child, cvt, pending_doc.take()) {
                    observables.push(o);
                }
            }
            _ => {}
        }
        pending_doc = None;
    }

    Some(SurfaceBlock {
        state,
        init,
        fairness,
        properties,
        defaults,
        actions,
        internal_actions,
        observables,
        span,
        doc,
    })
}

pub fn convert_property_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<Property> {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("name")
        .map(|n| ident_from_node(n, cvt))?;

    // The `body` field collects: an optional `always`/`eventually` token
    // (anonymous, so we detect by raw text) plus the predicate expression.
    let raw = cvt.text(node);
    let kind = if raw.contains("always") {
        PropertyKind::Safety
    } else if raw.contains("eventually") {
        PropertyKind::Liveness
    } else {
        PropertyKind::Safety
    };

    // Find the predicate: the last named child that is an expression-shaped
    // node under the `body` field.
    let body_expr_node = pick_property_body_expr(node, cvt);
    let body = match body_expr_node {
        Some(n) => convert_expr(n, cvt),
        None => Expr {
            kind: Box::new(ExprKind::LitBool(true)),
            span,
        },
    };
    Some(Property { name, kind, body, doc, span })
}

fn pick_property_body_expr<'b>(node: Node<'b>, cvt: &Cvt) -> Option<Node<'b>> {
    // The `body` field is `multiple` so iterate all matching children and
    // return the last expression-kind node (skipping `always`/`eventually`
    // marker tokens — those are anonymous and won't appear as named children).
    let mut last: Option<Node<'b>> = None;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                // The `name` field is also an identifier; skip it.
                if Some(child) == node.child_by_field_name("name") {
                    continue;
                }
                last = Some(child);
            }
            kind if kind.ends_with("_expr")
                || matches!(
                    kind,
                    "field_access"
                        | "index_expr"
                        | "indexed_path"
                        | "tuple_expr"
                        | "set_expr"
                        | "seq_literal"
                        | "map_literal"
                        | "record_expr"
                        | "string"
                        | "number"
                        | "bool_lit"
                        | "none_lit"
                        | "wildcard"
                        | "param_ref"
                        | "some_call"
                        | "comprehension"
                        | "qualified_name"
                ) =>
            {
                last = Some(child);
            }
            _ => {}
        }
    }
    last
}

// =============================================================================
// State fields
// =============================================================================

pub(super) fn convert_state_field(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> StateField {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("name")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let ty = node
        .child_by_field_name("type")
        .map(|n| convert_type_expr(n, cvt))
        .unwrap_or(Type {
            kind: TypeKind::Named(QualifiedName::new(vec![Ident::new("<missing>", span)])),
            span,
        });

    let mut kind = StateFieldKind::Plain;
    let mut retention: Option<RetentionClass> = None;
    let mut private = false;

    for c in cvt.all_named_children(node) {
        match c.kind() {
            "derived_modifier" => {
                let shape = c
                    .child_by_field_name("shape")
                    .map(|n| ident_from_node(n, cvt))
                    .and_then(|id| DerivedShape::from_name(&id.name));
                let of_type = c.child_by_field_name("of").map(|n| convert_type_expr(n, cvt));
                kind = StateFieldKind::Derived { shape, of_type };
            }
            "retention_modifier" => {
                if let Some(v) = c.child_by_field_name("value") {
                    let sv = convert_slot_value(v, cvt);
                    retention = retention_class_from_slot_value(&sv);
                }
            }
            "private_modifier" => private = true,
            _ => {}
        }
    }

    StateField { name, ty, kind, retention, private, doc, span }
}

fn retention_class_from_slot_value(v: &SlotValue) -> Option<RetentionClass> {
    match v {
        SlotValue::Retention(rv) => Some(match rv {
            RetentionValue::Ephemeral => RetentionClass::Ephemeral,
            RetentionValue::Transactional => RetentionClass::Transactional,
            RetentionValue::Audit { period } => RetentionClass::Audit { period: period.clone() },
            RetentionValue::Pii { class, ttl } => RetentionClass::Pii { class: *class, ttl: ttl.clone() },
            RetentionValue::Secret => RetentionClass::Secret,
        }),
        SlotValue::Unknown(id) => match id.name.as_str() {
            "ephemeral" => Some(RetentionClass::Ephemeral),
            "transactional" => Some(RetentionClass::Transactional),
            "secret" => Some(RetentionClass::Secret),
            _ => None,
        },
        _ => None,
    }
}

// =============================================================================
// init assignments
// =============================================================================

fn convert_init_assignment(node: Node, cvt: &mut Cvt) -> Option<InitAssignment> {
    if node.kind() != "assign_effect" {
        // init_block also accepts other effect statements; we only model
        // simple `name := expr` here.  Anything else is dropped silently
        // (the resolver will see no init for that name).
        return None;
    }
    let span = cvt.span(node);
    let target = node.child_by_field_name("target")?;
    if target.kind() != "identifier" {
        return None;
    }
    let name = ident_from_node(target, cvt);
    let value_node = node.child_by_field_name("value")?;
    let value = convert_expr(value_node, cvt);
    Some(InitAssignment { name, value, span })
}

// =============================================================================
// Slot assignments
// =============================================================================

pub(super) fn convert_slot_assign(node: Node, cvt: &mut Cvt) -> SlotAssign {
    let span = cvt.span(node);
    let kind = node
        .child_by_field_name("slot")
        .and_then(|n| SlotKind::from_name(cvt.text(n)))
        .unwrap_or(SlotKind::Idempotency);

    let value = node
        .child_by_field_name("value")
        .map(|v| convert_slot_value(v, cvt))
        .unwrap_or(SlotValue::Unknown(Ident::new("<missing>", span)));

    SlotAssign { kind, value, span }
}

fn convert_slot_value(node: Node, cvt: &mut Cvt) -> SlotValue {
    // node.kind() == "slot_value" wrapping one of slot_call|slot_set|slot_waiver
    let inner = cvt
        .all_named_children(node)
        .into_iter()
        .next()
        .unwrap_or(node);
    match inner.kind() {
        "slot_call" => convert_slot_call(inner, cvt),
        "slot_set" => convert_slot_set(inner, cvt),
        "slot_waiver" => {
            let reason = inner
                .child_by_field_name("reason")
                .map(|s| {
                    let raw = cvt.text(s);
                    raw.strip_prefix('"')
                        .and_then(|r| r.strip_suffix('"'))
                        .unwrap_or(raw)
                        .to_string()
                })
                .unwrap_or_default();
            SlotValue::Waived { reason, reason_span: cvt.span(inner) }
        }
        _ => SlotValue::Unknown(Ident::new(cvt.text(inner), cvt.span(inner))),
    }
}

fn convert_slot_call(node: Node, cvt: &mut Cvt) -> SlotValue {
    let name_node = node.child_by_field_name("name");
    let name_str = name_node.map(|n| cvt.text(n)).unwrap_or("");
    let span = cvt.span(node);

    // Collect args (as raw `call_args` children, each a call_arg).
    let arg_nodes: Vec<Node> = node
        .child_by_field_name("args")
        .map(|args| cvt.named_children_of(args, &["call_arg"]))
        .unwrap_or_default();
    let by_args: Vec<Ident> = node
        .child_by_field_name("by")
        .map(|by| {
            cvt.named_children_of(by, &["identifier"])
                .into_iter()
                .map(|n| ident_from_node(n, cvt))
                .collect()
        })
        .unwrap_or_default();

    // Helper closures.
    //
    // Self-review #3: positional `call_arg` has no `value` field in
    // the grammar — the expression IS the child. Named args are
    // wrapped in a `named_arg` node nested inside the call_arg, so we
    // skip those when picking positional args by index.
    let positional_nodes: Vec<Node> = arg_nodes
        .iter()
        .copied()
        .filter(|a| cvt.first_child(*a, "named_arg").is_none())
        .collect();
    let pos_arg_value = |idx: usize| -> Option<Node> {
        let a = positional_nodes.get(idx)?;
        a.child_by_field_name("value")
            .or_else(|| cvt.all_named_children(*a).into_iter().next())
    };
    let pos_arg_ident = |idx: usize| -> Option<Ident> {
        let v = pos_arg_value(idx)?;
        if v.kind() == "identifier_expr" {
            cvt.first_child(v, "identifier").map(|i| ident_from_node(i, cvt))
        } else if v.kind() == "identifier" {
            Some(ident_from_node(v, cvt))
        } else if v.kind() == "qualified_name" {
            cvt.first_child(v, "identifier").map(|i| ident_from_node(i, cvt))
        } else {
            None
        }
    };
    let pos_arg_text = |idx: usize| -> Option<String> {
        let v = pos_arg_value(idx)?;
        Some(cvt.text(v).to_string())
    };
    let named_arg = |name: &str| -> Option<Node> {
        for a in &arg_nodes {
            // The grammar layers `call_arg → named_arg → {name, value}`.
            // First look for a direct `name` field (legacy / future
            // shape), then descend into a `named_arg` child.
            if let Some(n) = a.child_by_field_name("name") {
                if cvt.text(n) == name {
                    return a.child_by_field_name("value");
                }
            }
            if let Some(na) = cvt.first_child(*a, "named_arg") {
                if let Some(n) = na.child_by_field_name("name") {
                    if cvt.text(n) == name {
                        return na.child_by_field_name("value");
                    }
                }
            }
        }
        None
    };

    match name_str {
        // --- idempotency ---
        "idempotent" if !by_args.is_empty() => {
            SlotValue::Idempotency(IdempotencyValue::IdempotentBy(by_args))
        }
        "idempotent" => SlotValue::Idempotency(IdempotencyValue::Idempotent),
        "at_most_once" => SlotValue::Idempotency(IdempotencyValue::AtMostOnce),
        "at_least_once" => SlotValue::Idempotency(IdempotencyValue::AtLeastOnce),

        // --- retention ---
        "ephemeral" => SlotValue::Retention(RetentionValue::Ephemeral),
        "transactional" => SlotValue::Retention(RetentionValue::Transactional),
        "secret" => SlotValue::Retention(RetentionValue::Secret),
        "audit" => {
            let period = named_arg("period")
                .and_then(|n| ident_text_of(n, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", span));
            SlotValue::Retention(RetentionValue::Audit { period })
        }
        "pii" => {
            let class = named_arg("class")
                .and_then(|n| ident_text_of(n, cvt))
                .and_then(|id| PiiClass::from_name(&id.name))
                .unwrap_or(PiiClass::Other);
            let ttl = named_arg("ttl")
                .and_then(|n| ident_text_of(n, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", span));
            SlotValue::Retention(RetentionValue::Pii { class, ttl })
        }

        // --- rate_limit ---
        "per_actor" => {
            let n = pos_arg_text(0).and_then(|t| t.parse::<u64>().ok()).unwrap_or(0);
            let period = named_arg("per")
                .and_then(|p| ident_text_of(p, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", span));
            SlotValue::RateLimit(RateLimitValue::PerActor { n, period })
        }
        "per_target" => {
            let arg = pos_arg_ident(0).unwrap_or_else(|| Ident::new("<missing>", span));
            let n = pos_arg_text(1).and_then(|t| t.parse::<u64>().ok()).unwrap_or(0);
            let period = named_arg("per")
                .and_then(|p| ident_text_of(p, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", span));
            SlotValue::RateLimit(RateLimitValue::PerTarget { arg, n, period })
        }
        "global" => {
            let n = pos_arg_text(0).and_then(|t| t.parse::<u64>().ok()).unwrap_or(0);
            let period = named_arg("per")
                .and_then(|p| ident_text_of(p, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", span));
            SlotValue::RateLimit(RateLimitValue::Global { n, period })
        }
        "unlimited" => SlotValue::RateLimit(RateLimitValue::Unlimited),

        // --- observability ---
        "caller_only" => {
            let events = arg_nodes.iter().filter_map(|a| {
                let v = a.child_by_field_name("value")
                    .or_else(|| cvt.all_named_children(*a).into_iter().next())?;
                ident_text_of(v, cvt)
            }).collect();
            SlotValue::Observability(ObservabilityValue::CallerOnly(events))
        }
        "broadcast" => {
            let events = arg_nodes.iter().filter_map(|a| {
                let v = a.child_by_field_name("value")
                    .or_else(|| cvt.all_named_children(*a).into_iter().next())?;
                ident_text_of(v, cvt)
            }).collect();
            SlotValue::Observability(ObservabilityValue::Broadcast(events))
        }
        "target" => {
            let arg = pos_arg_ident(0).unwrap_or_else(|| Ident::new("<missing>", span));
            let events = arg_nodes.iter().skip(1).filter_map(|a| {
                let v = a.child_by_field_name("value")
                    .or_else(|| cvt.all_named_children(*a).into_iter().next())?;
                ident_text_of(v, cvt)
            }).collect();
            SlotValue::Observability(ObservabilityValue::Target { arg, events })
        }
        "silent" => SlotValue::Observability(ObservabilityValue::Silent),

        // --- availability ---
        n if AvailabilityValue::from_name(n).is_some() => {
            SlotValue::Availability(AvailabilityValue::from_name(n).unwrap())
        }

        // --- freshness ---
        "strong" => SlotValue::Freshness(FreshnessValue::Strong),
        "not_applicable" => SlotValue::Freshness(FreshnessValue::NotApplicable),
        "bounded" => {
            let epoch = pos_arg_ident(0).unwrap_or_else(|| Ident::new("<missing>", span));
            let n = named_arg("n")
                .and_then(|p| Some(cvt.text(p).parse::<u64>().ok()?))
                .unwrap_or(0);
            SlotValue::Freshness(FreshnessValue::Bounded { epoch, n })
        }
        "eventual" => {
            let epoch = pos_arg_ident(0);
            SlotValue::Freshness(FreshnessValue::Eventual { epoch })
        }
        "stale_while_revalidate" => {
            let epoch = pos_arg_ident(0).unwrap_or_else(|| Ident::new("<missing>", span));
            let n = named_arg("n")
                .and_then(|p| Some(cvt.text(p).parse::<u64>().ok()?))
                .unwrap_or(0);
            SlotValue::Freshness(FreshnessValue::StaleWhileRevalidate { epoch, n })
        }

        // Single-channel auth_channel form: `auth_channel : signed_request`.
        n if AuthChannelTag::from_name(n).is_some() => {
            SlotValue::AuthChannel(AuthChannelValue {
                channels: vec![AuthChannelTag::from_name(n).unwrap()],
                set_form: false,
            })
        }

        _ => SlotValue::Unknown(Ident::new(name_str, span)),
    }
}

fn convert_slot_set(node: Node, cvt: &mut Cvt) -> SlotValue {
    let channels: Vec<AuthChannelTag> = cvt
        .named_children_of(node, &["identifier"])
        .into_iter()
        .filter_map(|n| AuthChannelTag::from_name(cvt.text(n)))
        .collect();
    SlotValue::AuthChannel(AuthChannelValue { channels, set_form: true })
}

fn ident_text_of(node: Node, cvt: &Cvt) -> Option<Ident> {
    match node.kind() {
        "identifier" => Some(ident_from_node(node, cvt)),
        "identifier_expr" => {
            let id = cvt.first_child(node, "identifier")?;
            Some(ident_from_node(id, cvt))
        }
        "qualified_name" => {
            let qn = convert_qualified_name(node, cvt);
            qn.segments.into_iter().next()
        }
        _ => None,
    }
}

// =============================================================================
// Action declarations (surface-level; with slots and raises)
// =============================================================================

pub(super) fn convert_action_decl(
    node: Node,
    cvt: &mut Cvt,
    is_internal: bool,
    doc: Option<String>,
) -> Option<ActionDecl> {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("name")
        .map(|n| ident_from_node(n, cvt))?;
    let return_ty = node
        .child_by_field_name("return_type")
        .map(|n| convert_type_expr(n, cvt));
    let params: Vec<Param> = if let Some(plist) = cvt.first_child(node, "param_list") {
        convert_param_list(plist, cvt).into_iter().map(Into::into).collect()
    } else {
        Vec::new()
    };
    let actor = if let Some(by) = cvt.first_child(node, "by_clause") {
        let var = by
            .child_by_field_name("var")
            .map(|n| ident_from_node(n, cvt))
            .unwrap_or_else(|| Ident::new("<missing>", span));
        let actor_ty = by
            .child_by_field_name("type")
            .map(|n| ident_from_node(n, cvt))
            .unwrap_or_else(|| Ident::new("<missing>", span));
        let aspan = var.span;
        ActorBinder { name: var, actor_ty, span: aspan }
    } else {
        ActorBinder {
            name: Ident::new("<no-actor>", span),
            actor_ty: Ident::new("<missing>", span),
            span,
        }
    };
    let when_pre = cvt
        .first_child(node, "when_clause")
        .and_then(|w| w.child_by_field_name("guard"))
        .map(|g| convert_expr(g, cvt));
    let raises: Vec<RaisesClause> = cvt
        .first_child(node, "raises_block")
        .map(|rb| {
            cvt.named_children_of(rb, &["raises_entry"])
                .into_iter()
                .map(|e| convert_raises_entry(e, cvt))
                .collect()
        })
        .unwrap_or_default();
    let slots: Vec<SlotAssign> = cvt
        .named_children_of(node, &["slot_assign"])
        .into_iter()
        .map(|s| convert_slot_assign(s, cvt))
        .collect();
    let body = cvt
        .first_child(node, "then_block")
        .map(|tb| convert_effect_block(tb, cvt))
        .unwrap_or(EffectBlock { stmts: Vec::new(), span });

    Some(ActionDecl {
        name,
        params,
        return_ty,
        actor,
        when_pre,
        raises,
        slots,
        body,
        doc,
        span,
        is_internal,
    })
}

fn convert_raises_entry(node: Node, cvt: &mut Cvt) -> RaisesClause {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("name")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let guard = node
        .child_by_field_name("guard")
        .map(|g| convert_expr(g, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitBool(true)),
            span,
        });
    RaisesClause { name, guard, span }
}

// =============================================================================
// Effect blocks and statements
// =============================================================================

pub(super) fn convert_effect_block(node: Node, cvt: &mut Cvt) -> EffectBlock {
    let span = cvt.span(node);
    let stmts = cvt
        .all_named_children(node)
        .into_iter()
        .filter_map(|c| convert_effect_stmt(c, cvt))
        .collect();
    EffectBlock { stmts, span }
}

pub(super) fn convert_effect_stmt(node: Node, cvt: &mut Cvt) -> Option<EffectStmt> {
    let span = cvt.span(node);
    Some(match node.kind() {
        "assign_effect" => {
            let target = convert_expr(node.child_by_field_name("target")?, cvt);
            let value = convert_expr(node.child_by_field_name("value")?, cvt);
            EffectStmt::Assign { target, value, span }
        }
        "compound_assign_effect" => {
            let op = node
                .child_by_field_name("op")
                .map(|n| cvt.text(n))
                .unwrap_or("+=");
            let target = convert_expr(node.child_by_field_name("target")?, cvt);
            let value = convert_expr(node.child_by_field_name("value")?, cvt);
            if op == "-=" {
                EffectStmt::SubAssign { target, value, span }
            } else {
                EffectStmt::AddAssign { target, value, span }
            }
        }
        "map_assign_effect" => {
            let target = convert_expr(node.child_by_field_name("target")?, cvt);
            let value = convert_expr(node.child_by_field_name("value")?, cvt);
            EffectStmt::Assign { target, value, span }
        }
        "delete_effect" => {
            let target = convert_expr(node.child_by_field_name("target")?, cvt);
            EffectStmt::DeleteKey { target, span }
        }
        "snoc_effect" => {
            let target = convert_expr(node.child_by_field_name("target")?, cvt);
            let value = convert_expr(node.child_by_field_name("value")?, cvt);
            EffectStmt::SeqSnoc { target, value, span }
        }
        "emit_effect" => {
            let event = ident_from_node(node.child_by_field_name("event")?, cvt);
            let args: Vec<CallArg> = cvt
                .named_children_of(node, &["call_arg"])
                .into_iter()
                .map(|a| convert_call_arg(a, cvt))
                .collect();
            EffectStmt::Emit { event, args, span }
        }
        "return_effect" => {
            let value = convert_expr(node.child_by_field_name("value")?, cvt);
            EffectStmt::Return { value, span }
        }
        "let_effect" => {
            let name = ident_from_node(node.child_by_field_name("name")?, cvt);
            let value = convert_expr(node.child_by_field_name("value")?, cvt);
            EffectStmt::Let { name, value, span }
        }
        "if_effect" => {
            let cond = convert_expr(node.child_by_field_name("guard")?, cvt);
            let then_label = node
                .child_by_field_name("then_label")
                .and_then(|n| convert_branch_label(n, cvt));
            // children = effect statements + optional else_clause
            let mut then_stmts: Vec<EffectStmt> = Vec::new();
            let mut else_block: Option<EffectBlock> = None;
            let mut else_label: Option<BranchLabel> = None;
            for c in cvt.all_named_children(node) {
                if c.kind() == "else_clause" {
                    else_label = c
                        .child_by_field_name("else_label")
                        .and_then(|n| convert_branch_label(n, cvt));
                    let ebspan = cvt.span(c);
                    let estmts: Vec<EffectStmt> = cvt
                        .all_named_children(c)
                        .into_iter()
                        .filter_map(|n| convert_effect_stmt(n, cvt))
                        .collect();
                    else_block = Some(EffectBlock { stmts: estmts, span: ebspan });
                } else if let Some(s) = convert_effect_stmt(c, cvt) {
                    then_stmts.push(s);
                }
            }
            let then_block = EffectBlock { stmts: then_stmts, span };
            EffectStmt::IfElse {
                cond,
                then_label,
                then_block,
                else_label,
                else_block,
                span,
            }
        }
        "for_effect" => {
            let name = ident_from_node(node.child_by_field_name("var")?, cvt);
            let domain = convert_expr(node.child_by_field_name("iter")?, cvt);
            let body_stmts: Vec<EffectStmt> = cvt
                .all_named_children(node)
                .into_iter()
                .filter_map(|c| {
                    if c == node.child_by_field_name("var").unwrap()
                        || c == node.child_by_field_name("iter").unwrap()
                    {
                        None
                    } else {
                        convert_effect_stmt(c, cvt)
                    }
                })
                .collect();
            EffectStmt::For {
                name,
                domain,
                body: EffectBlock { stmts: body_stmts, span },
                span,
            }
        }
        "match_effect" => {
            let scrutinee = convert_expr(node.child_by_field_name("scrutinee")?, cvt);
            let arms: Vec<EffectMatchArm> = cvt
                .named_children_of(node, &["match_arm"])
                .into_iter()
                .map(|a| convert_effect_match_arm(a, cvt))
                .collect();
            EffectStmt::Match { scrutinee, arms, span }
        }
        "if_let_effect" => {
            let name = ident_from_node(node.child_by_field_name("binding")?, cvt);
            let source = convert_expr(node.child_by_field_name("value")?, cvt);
            let stmts: Vec<EffectStmt> = cvt
                .all_named_children(node)
                .into_iter()
                .filter_map(|c| {
                    if Some(c) == node.child_by_field_name("binding")
                        || Some(c) == node.child_by_field_name("value")
                    {
                        None
                    } else {
                        convert_effect_stmt(c, cvt)
                    }
                })
                .collect();
            EffectStmt::IfLetSome {
                name,
                source,
                then_block: EffectBlock { stmts, span },
                else_block: None,
                span,
            }
        }
        "sends_effect" => {
            // Self-review must-fix #4: preserve channel destination
            // facts so the obligation pass can build `sends_on(...)`.
            let message = ident_from_node(node.child_by_field_name("msg")?, cvt);
            let args: Vec<CallArg> = cvt
                .named_children_of(node, &["call_arg"])
                .into_iter()
                .map(|a| convert_call_arg(a, cvt))
                .collect();
            // Grammar emits one of three destinations (per §7.2.1):
            //   - `to <ChannelName>`  → `dest` field is an identifier
            //   - `to <Component>`    → `dest` field is a qualified_name
            //   - `to <Component>[<idExpr>]` → `dest` is an index_expr
            // We tolerate all shapes; absence is also valid (shorthand
            // when only one channel exists).
            let mut to_channel = None;
            let mut to_component = None;
            let mut to_instance = None;
            if let Some(dest) = node.child_by_field_name("dest") {
                match dest.kind() {
                    "identifier" => to_channel = Some(ident_from_node(dest, cvt)),
                    "qualified_name" => to_component = Some(super::convert_qualified_name(dest, cvt)),
                    _ => {
                        // For more complex shapes (index_expr, etc.) we
                        // fall back to the channel-name interpretation
                        // by extracting the text. The obligation pass
                        // can refine; this preserves the fact that a
                        // destination was specified.
                        to_instance = Some(convert_expr(dest, cvt));
                    }
                }
            }
            EffectStmt::Sends { message, args, to_channel, to_component, to_instance, span }
        }
        _ => return None,
    })
}

fn convert_effect_match_arm(node: Node, cvt: &mut Cvt) -> EffectMatchArm {
    let span = cvt.span(node);
    let pattern = node
        .child_by_field_name("pattern")
        .map(|p| convert_match_pattern(p, cvt))
        .unwrap_or(MatchPattern::Wildcard);
    let body = node
        .child_by_field_name("body")
        .map(|b| {
            if b.kind() == "then_block" {
                convert_effect_block(b, cvt)
            } else {
                // Single expression body; wrap as a single Return-like... but
                // match_arm bodies in our grammar are expressions. For statement
                // form we represent the single expr as no-op, since the parser
                // doesn't actually emit `match_effect` arms with effect-block bodies
                // in the v0.10 examples. Tolerant: empty block.
                EffectBlock { stmts: Vec::new(), span: cvt.span(b) }
            }
        })
        .unwrap_or(EffectBlock { stmts: Vec::new(), span });
    EffectMatchArm { pattern, body, span }
}

fn convert_match_pattern(node: Node, cvt: &mut Cvt) -> MatchPattern {
    // match_pattern: binding?: identifier; child: identifier | wildcard.
    let binding = node
        .child_by_field_name("binding")
        .map(|n| ident_from_node(n, cvt));
    if let Some(child) = cvt.all_named_children(node).into_iter().find(|c| {
        Some(*c) != node.child_by_field_name("binding")
    }) {
        match child.kind() {
            "wildcard" => MatchPattern::Wildcard,
            "identifier" => match cvt.text(child) {
                "None" => MatchPattern::None_,
                "Some" => MatchPattern::Some_(binding.unwrap_or_else(|| {
                    Ident::new("_", cvt.span(node))
                })),
                _ => MatchPattern::Wildcard,
            },
            _ => MatchPattern::Wildcard,
        }
    } else {
        MatchPattern::Wildcard
    }
}

fn convert_branch_label(node: Node, cvt: &Cvt) -> Option<BranchLabel> {
    let span = cvt.span(node);
    let id = node.child_by_field_name("name")?;
    Some(BranchLabel { name: ident_from_node(id, cvt), span })
}
