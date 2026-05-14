//! Convert substrate and partial-substrate blocks.

use super::convert_expr::convert_expr;
use super::convert_surface::{
    convert_action_decl, convert_effect_block, convert_slot_assign, convert_state_field,
};
use super::{
    convert_fairness_decl, convert_param_list, convert_qualified_name, convert_type_expr,
    doc_string_text, ident_from_node, parse_fairness_target, Cvt,
};
use surfacide_ast::decl::Param;
use surfacide_ast::expr::{Expr, ExprKind, PathAccessor, PathExpr};
use surfacide_ast::surface::{
    BranchLabel, EffectBlock, InitAssignment, RaisesClause, StateField,
};
use surfacide_ast::substrate::*;
use surfacide_ast::*;
use tree_sitter::Node;

pub fn convert_substrate_block(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<SubstrateBlock> {
    let block = build_substrate_block(node, cvt, doc, /*owns=*/ Vec::new());
    Some(block.0)
}

pub fn convert_partial_substrate_block(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<PartialSubstrateBlock> {
    // `partial_substrate_block.field` is the `owns { … }` identifier list.
    let owns: Vec<Ident> = {
        let mut out = Vec::new();
        let mut cursor = node.walk();
        for c in node.children_by_field_name("field", &mut cursor) {
            if c.kind() == "identifier" {
                out.push(ident_from_node(c, cvt));
            }
        }
        out
    };
    let (block, _) = build_substrate_block(node, cvt, doc, owns.clone());
    Some(PartialSubstrateBlock { block, owns })
}

fn build_substrate_block(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
    _partial_owns: Vec<Ident>,
) -> (SubstrateBlock, ()) {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("name")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let realizes_target = node
        .child_by_field_name("surface")
        .map(|n| convert_qualified_name(n, cvt));

    let mut components = Vec::new();
    let mut replicates = Vec::new();
    let mut channels = Vec::new();
    let mut auxiliary: Option<AuxiliaryBlock> = None;
    let mut authentication: Option<AuthenticationBlock> = None;
    let mut maps: Option<MapsBlock> = None;
    let mut realizes: Option<RealizesBlock> = None;
    let mut internal: Option<InternalBlock> = None;
    let mut fairness = Vec::new();
    let mut epochs = Vec::new();
    let mut acknowledged: Option<surfacide_ast::compose::AcknowledgedBlock> = None;
    let mut pending_doc: Option<String> = None;

    for child in cvt.all_named_children(node) {
        match child.kind() {
            "doc_string" => {
                pending_doc = Some(doc_string_text(cvt.text(child)));
                continue;
            }
            "component_decl" => {
                if let Some(c) = convert_component(child, cvt, pending_doc.take()) {
                    components.push(c);
                }
            }
            "replicate_decl" => {
                if let Some(r) = convert_replicate(child, cvt, pending_doc.take()) {
                    replicates.push(r);
                }
            }
            "channel_decl" => {
                if let Some(c) = convert_channel_decl(child, cvt) {
                    channels.push(c);
                }
            }
            "auxiliary_block" => {
                auxiliary = Some(convert_auxiliary_block(child, cvt));
            }
            "authentication_block" => {
                authentication = Some(convert_authentication_block(child, cvt));
            }
            "maps_block" => {
                maps = Some(convert_maps_block(child, cvt));
            }
            "realizes_block" => {
                realizes = Some(convert_realizes_block(child, cvt));
            }
            "internal_block" => {
                internal = Some(convert_internal_block(child, cvt));
            }
            "fairness_decl" => {
                if let Some(f) = convert_fairness_decl(child, cvt) {
                    fairness.push(f);
                }
            }
            "epoch_decl" => {
                if let Some(e) = convert_epoch_decl(child, cvt) {
                    epochs.push(e);
                }
            }
            "acknowledged_block" => {
                // v0.10 §15.3: acknowledged blocks may live inside any
                // substrate or compose that an obligation was derived
                // from. Substrate-side acks are surfaced here so the
                // obligation pass can find them.
                acknowledged = Some(
                    super::convert_compose::convert_acknowledged_block(child, cvt),
                );
            }
            _ => {}
        }
        pending_doc = None;
    }

    (
        SubstrateBlock {
            name,
            realizes_target,
            components,
            replicates,
            channels,
            auxiliary,
            authentication,
            maps,
            realizes,
            internal,
            fairness,
            epochs,
            acknowledged,
            doc,
            span,
        },
        (),
    )
}

// =============================================================================
// Components and replicates
// =============================================================================

fn convert_component(node: Node, cvt: &mut Cvt, doc: Option<String>) -> Option<Component> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let mut state: Vec<StateField> = Vec::new();
    let mut init: Vec<InitAssignment> = Vec::new();
    let mut actions: Vec<ComponentAction> = Vec::new();
    let mut receives: Vec<ReceivesHandler> = Vec::new();
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
                        "state_field" => state.push(convert_state_field(f, cvt, inner_doc.take())),
                        _ => {}
                    }
                }
            }
            "init_block" => {
                for f in cvt.all_named_children(child) {
                    if f.kind() == "assign_effect" {
                        let target = match f.child_by_field_name("target") {
                            Some(t) if t.kind() == "identifier" => ident_from_node(t, cvt),
                            _ => continue,
                        };
                        if let Some(v) = f.child_by_field_name("value") {
                            init.push(InitAssignment {
                                name: target,
                                value: convert_expr(v, cvt),
                                span: cvt.span(f),
                            });
                        }
                    }
                }
            }
            "component_action_decl" => {
                if let Some(a) = convert_component_action_decl(child, cvt, pending_doc.take()) {
                    actions.push(a);
                }
            }
            "receives_decl" => {
                if let Some(r) = convert_receives_decl(child, cvt, pending_doc.take()) {
                    receives.push(r);
                }
            }
            _ => {}
        }
        pending_doc = None;
    }
    Some(Component { name, state, init, actions, receives, doc, span })
}

fn convert_replicate(node: Node, cvt: &mut Cvt, doc: Option<String>) -> Option<ReplicateBlock> {
    // replicate_decl has fields name, id, id_type, id_set; children are
    // the same as a component (state_block, init_block, component_action_decl,
    // receives_decl, doc_string).
    let id_param = node
        .child_by_field_name("id")
        .map(|n| ident_from_node(n, cvt))?;
    let id_ty = node
        .child_by_field_name("id_type")
        .map(|n| convert_type_expr(n, cvt))?;
    let id_domain = node
        .child_by_field_name("id_set")
        .map(|n| convert_expr(n, cvt))?;
    let comp = convert_component(node, cvt, doc)?;
    Some(ReplicateBlock { component: comp, id_param, id_ty, id_domain })
}

fn convert_component_action_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<ComponentAction> {
    // `component_action_decl` schema mirrors `action_decl` but slot_assign
    // is absent (substrate component actions inherit slot semantics from
    // realizes-targets).  Reuse the surface-level converter and discard
    // any stray slot fields.
    let a = convert_action_decl(node, cvt, /*is_internal=*/ false, doc)?;
    Some(ComponentAction {
        name: a.name,
        params: a.params,
        return_ty: a.return_ty,
        when_pre: a.when_pre,
        raises: a.raises,
        body: a.body,
        doc: a.doc,
        span: a.span,
    })
}

fn convert_receives_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<ReceivesHandler> {
    let span = cvt.span(node);
    // Grammar field is `msg`, not `message`/`name`. Tolerate both.
    let message = node
        .child_by_field_name("msg")
        .or_else(|| node.child_by_field_name("message"))
        .or_else(|| node.child_by_field_name("name"))
        .map(|n| ident_from_node(n, cvt))?;
    let from_channel = node
        .child_by_field_name("channel")
        .or_else(|| node.child_by_field_name("from"))
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    // Grammar emits `param` children directly (no `param_list` wrapper
    // for receives_decl). Iterate.
    let params: Vec<Param> = cvt
        .named_children_of(node, &["param"])
        .into_iter()
        .map(|p| {
            let pspan = cvt.span(p);
            let name = p
                .child_by_field_name("name")
                .map(|n| ident_from_node(n, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", pspan));
            let ty = p
                .child_by_field_name("type")
                .map(|n| convert_type_expr(n, cvt))
                .unwrap_or(surfacide_ast::Type {
                    kind: surfacide_ast::TypeKind::Named(
                        surfacide_ast::QualifiedName::new(vec![Ident::new("<missing>", pspan)]),
                    ),
                    span: pspan,
                });
            Param { name, ty, span: pspan }
        })
        .collect();
    let when_pre = cvt
        .first_child(node, "when_clause")
        .and_then(|w| w.child_by_field_name("guard"))
        .map(|g| convert_expr(g, cvt));
    let body = cvt
        .first_child(node, "then_block")
        .map(|tb| convert_effect_block(tb, cvt))
        .unwrap_or(EffectBlock { stmts: Vec::new(), span });
    Some(ReceivesHandler { message, params, from_channel, when_pre, body, doc, span })
}

// =============================================================================
// Channels
// =============================================================================

pub(super) fn convert_channel_decl(node: Node, cvt: &mut Cvt) -> Option<ChannelDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let (from_comp, from_mult) = node
        .child_by_field_name("from")
        .map(|n| parse_channel_endpoint(n, cvt))
        .unwrap_or((QualifiedName::new(vec![Ident::new("<missing>", span)]), ChannelMultiplicity::One));
    let (to_comp, to_mult) = node
        .child_by_field_name("to")
        .map(|n| parse_channel_endpoint(n, cvt))
        .unwrap_or((QualifiedName::new(vec![Ident::new("<missing>", span)]), ChannelMultiplicity::One));
    Some(ChannelDecl { name, from_comp, from_mult, to_comp, to_mult, span })
}

fn parse_channel_endpoint(node: Node, cvt: &Cvt) -> (QualifiedName, ChannelMultiplicity) {
    let span = cvt.span(node);
    let raw = cvt.text(node).trim();
    let idents: Vec<Ident> = cvt
        .named_children_of(node, &["identifier"])
        .into_iter()
        .map(|n| ident_from_node(n, cvt))
        .collect();
    if node.kind() == "identifier" {
        return (
            QualifiedName::new(vec![ident_from_node(node, cvt)]),
            ChannelMultiplicity::One,
        );
    }

    let mult = if let Some(open) = raw.find('[') {
        if let Some(close) = raw[open..].find(']') {
            let inside = raw[open + 1..open + close].trim();
            if inside == "*" {
                ChannelMultiplicity::Star
            } else {
                ChannelMultiplicity::PairwiseId(Ident::new(inside, span))
            }
        } else {
            ChannelMultiplicity::One
        }
    } else {
        ChannelMultiplicity::One
    };
    let comp_text = raw.split('[').next().unwrap_or(raw).trim();
    let segs: Vec<Ident> = comp_text
        .split('.')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            idents
                .iter()
                .find(|i| i.name == s)
                .cloned()
                .unwrap_or_else(|| Ident::new(s, span))
        })
        .collect();
    let qn = if segs.is_empty() {
        QualifiedName::new(vec![Ident::new(comp_text, span)])
    } else {
        QualifiedName::new(segs)
    };
    (qn, mult)
}

// =============================================================================
// Auxiliary
// =============================================================================

fn convert_auxiliary_block(node: Node, cvt: &mut Cvt) -> AuxiliaryBlock {
    let span = cvt.span(node);
    let mut vars: Vec<AuxiliaryVar> = Vec::new();
    let mut pending_doc: Option<String> = None;
    for c in cvt.all_named_children(node) {
        match c.kind() {
            "doc_string" => pending_doc = Some(doc_string_text(cvt.text(c))),
            "aux_decl" => {
                vars.push(convert_aux_decl(c, cvt));
                pending_doc = None;
            }
            _ => {}
        }
    }
    AuxiliaryBlock { vars, span }
}

fn convert_aux_decl(node: Node, cvt: &mut Cvt) -> AuxiliaryVar {
    let span = cvt.span(node);
    let kind = match node.child_by_field_name("kind").map(|n| cvt.text(n)).unwrap_or("history") {
        "prophecy" => AuxiliaryKind::Prophecy,
        _ => AuxiliaryKind::History,
    };
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
    let init = match node.child_by_field_name("init") {
        Some(n) if cvt.text(n).trim() == "*" => None,
        Some(n) => Some(convert_expr(n, cvt)),
        None => None,
    };
    let cross_visible = node.child_by_field_name("cross_visible").is_some();
    let invariant = node.child_by_field_name("invariant").map(|n| convert_expr(n, cvt));
    AuxiliaryVar { kind, name, ty, init, cross_visible, invariant, span }
}

// =============================================================================
// Authentication
// =============================================================================

fn convert_authentication_block(node: Node, cvt: &mut Cvt) -> AuthenticationBlock {
    let span = cvt.span(node);
    let mappings: Vec<AuthenticationMapping> = cvt
        .named_children_of(node, &["auth_mapping"])
        .into_iter()
        .map(|m| convert_auth_mapping(m, cvt))
        .collect();
    AuthenticationBlock { mappings, span }
}

fn convert_auth_mapping(node: Node, cvt: &mut Cvt) -> AuthenticationMapping {
    let span = cvt.span(node);
    let surface_action = node
        .child_by_field_name("action")
        .map(|n| convert_qualified_name(n, cvt))
        .unwrap_or(QualifiedName::new(vec![Ident::new("<missing>", span)]));
    let rhs = match node.child_by_field_name("source") {
        Some(s) => match s.kind() {
            "param_ref" => {
                if let Some(id) = cvt.first_child(s, "identifier") {
                    AuthRhs::Param(ident_from_node(id, cvt))
                } else {
                    AuthRhs::Param(Ident::new("<missing>", cvt.span(s)))
                }
            }
            "system" => AuthRhs::System,
            _ => {
                // indexed_path or similar: lower into PathExpr by reusing the
                // path-shaped expression conversion, then strip down to PathExpr.
                let e = convert_expr(s, cvt);
                if let ExprKind::Path(p) = *e.kind {
                    AuthRhs::Path(p)
                } else {
                    AuthRhs::Path(PathExpr {
                        head: Ident::new(cvt.text(s), cvt.span(s)),
                        accessors: Vec::new(),
                    })
                }
            }
        },
        None => AuthRhs::System,
    };
    AuthenticationMapping { surface_action, rhs, span }
}

// =============================================================================
// Maps
// =============================================================================

fn convert_maps_block(node: Node, cvt: &mut Cvt) -> MapsBlock {
    let span = cvt.span(node);
    let mappings: Vec<MapsEntry> = cvt
        .named_children_of(node, &["maps_entry"])
        .into_iter()
        .map(|e| {
            let espan = cvt.span(e);
            let field = e
                .child_by_field_name("field")
                .map(|n| ident_from_node(n, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", espan));
            let value = e
                .child_by_field_name("value")
                .map(|n| convert_expr(n, cvt))
                .unwrap_or(Expr {
                    kind: Box::new(ExprKind::LitNone),
                    span: espan,
                });
            MapsEntry { field, value, span: espan }
        })
        .collect();
    MapsBlock { mappings, span }
}

// =============================================================================
// Realizes
// =============================================================================

pub(super) fn convert_realizes_block(node: Node, cvt: &mut Cvt) -> RealizesBlock {
    let span = cvt.span(node);
    let mut clauses: Vec<RealizesClause> = Vec::new();
    for entry in cvt.named_children_of(node, &["realizes_entry"]) {
        let inner = match cvt.all_named_children(entry).into_iter().next() {
            Some(n) => n,
            None => continue,
        };
        match inner.kind() {
            "realizes_clause" => clauses.push(convert_realizes_clause(inner, cvt, Vec::new())),
            "for_some_realizes" => {
                let var = inner
                    .child_by_field_name("var")
                    .map(|n| ident_from_node(n, cvt));
                let set_expr = inner
                    .child_by_field_name("set")
                    .map(|n| convert_expr(n, cvt));
                let for_some = match (var, set_expr) {
                    (Some(v), Some(s)) => vec![(v, s)],
                    _ => Vec::new(),
                };
                if let Some(rc) = cvt.first_child(inner, "realizes_clause") {
                    clauses.push(convert_realizes_clause(rc, cvt, for_some));
                }
            }
            _ => {}
        }
    }
    RealizesBlock { clauses, span }
}

fn convert_realizes_clause(
    node: Node,
    cvt: &mut Cvt,
    for_some: Vec<(Ident, Expr)>,
) -> RealizesClause {
    let span = cvt.span(node);
    let action_id = node
        .child_by_field_name("action")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let surface_action = QualifiedName::new(vec![Ident::new("surface", span), action_id]);

    // Selectors: 0+ channel_selector. First → ChannelSelector, second (if any)
    // → BranchLabel. v0.10.1 examples use both forms.
    let selectors: Vec<Node> = cvt.named_children_of(node, &["channel_selector"]);
    let mut sel_iter = selectors.iter();
    let channel_selector = sel_iter
        .next()
        .map(|s| selector_from_node(*s, cvt))
        .unwrap_or(ChannelSelector::None);
    let branch_label = sel_iter.next().and_then(|s| selector_to_branch_label(*s, cvt));

    // Args: any expression children that aren't `when_clause`/`channel_selector`/`target`-shaped.
    let target_node = node.child_by_field_name("target");
    let mut args: Vec<Ident> = Vec::new();
    for c in cvt.all_named_children(node) {
        if Some(c) == target_node {
            continue;
        }
        if matches!(c.kind(), "channel_selector" | "when_clause") {
            continue;
        }
        // The action identifier is exposed via the `action` field.
        if Some(c) == node.child_by_field_name("action") {
            continue;
        }
        // Try to extract a bare identifier for each arg position.
        match c.kind() {
            "identifier" => args.push(ident_from_node(c, cvt)),
            "identifier_expr" => {
                if let Some(id) = cvt.first_child(c, "identifier") {
                    args.push(ident_from_node(id, cvt));
                }
            }
            _ => {
                // Non-identifier args (literals or expressions) lose their
                // identifier-ness; we skip them and let the resolver flag.
            }
        }
    }

    let target = match target_node {
        Some(t) => match t.kind() {
            "EXTERNAL" => RealizesTarget::External,
            "stutter" => RealizesTarget::Stutter,
            _ => parse_realization_path(t, cvt),
        },
        None => RealizesTarget::External,
    };

    let when_guard = cvt
        .first_child(node, "when_clause")
        .and_then(|w| w.child_by_field_name("guard"))
        .map(|g| convert_expr(g, cvt));

    RealizesClause {
        surface_action,
        args,
        channel_selector,
        branch_label,
        target,
        when_guard,
        for_some,
        span,
    }
}

fn selector_from_node(node: Node, cvt: &Cvt) -> ChannelSelector {
    let span = cvt.span(node);
    if node.child_by_field_name("star").is_some() {
        return ChannelSelector::Star { span };
    }
    if let Some(id) = node.child_by_field_name("name") {
        return ChannelSelector::Specific(ident_from_node(id, cvt));
    }
    ChannelSelector::None
}

fn selector_to_branch_label(node: Node, cvt: &Cvt) -> Option<BranchLabel> {
    let span = cvt.span(node);
    let id = node.child_by_field_name("name")?;
    Some(BranchLabel { name: ident_from_node(id, cvt), span })
}

fn parse_realization_path(node: Node, cvt: &Cvt) -> RealizesTarget {
    // realization_path's named children are arbitrary expressions (the grammar
    // is permissive). We re-extract the identifier list and `[id]` selector
    // from the raw text.
    let span = cvt.span(node);
    let raw = cvt.text(node).trim();

    let idents: Vec<Ident> = collect_identifier_descendants(node, cvt);

    // Detect `[id]` (replicate selector).
    let bracket = raw.find('[');
    let (comp_text, replicate_id) = if let Some(b) = bracket {
        let close = raw[b..].find(']').unwrap_or(raw.len() - b);
        let inside = raw[b + 1..b + close].trim();
        let comp = raw[..b].trim_end();
        let id = if inside == "*" {
            None
        } else {
            // Try to find span-bearing ident
            Some(
                idents
                    .iter()
                    .find(|i| i.name == inside)
                    .cloned()
                    .unwrap_or_else(|| Ident::new(inside, span)),
            )
        };
        (comp.to_string(), id)
    } else {
        (raw.to_string(), None)
    };

    // The text after `]` (or after the comp_text) is `.action`.
    let after = if let Some(b) = bracket {
        let close = raw[b..].find(']').map(|p| b + p + 1).unwrap_or(raw.len());
        raw[close..].trim_start_matches('.').trim()
    } else {
        ""
    };
    let action_text = if after.is_empty() {
        // `Comp.action` (no brackets) — last dot segment.
        comp_text
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        after.to_string()
    };

    let component_text = if after.is_empty() {
        comp_text
            .rsplit_once('.')
            .map(|(c, _)| c.to_string())
            .unwrap_or_else(|| comp_text.clone())
    } else {
        comp_text.clone()
    };

    let component = qn_from_text(&component_text, &idents, span);
    let action = idents
        .iter()
        .find(|i| i.name == action_text)
        .cloned()
        .unwrap_or_else(|| Ident::new(action_text, span));

    RealizesTarget::Action { component, replicate_id, action }
}

fn collect_identifier_descendants<'b>(node: Node<'b>, cvt: &Cvt) -> Vec<Ident> {
    let mut out = Vec::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if n.kind() == "identifier" {
            out.push(ident_from_node(n, cvt));
        }
        let mut cursor = n.walk();
        for c in n.named_children(&mut cursor) {
            stack.push(c);
        }
    }
    out
}

fn qn_from_text(text: &str, idents: &[Ident], span: Span) -> QualifiedName {
    let segs: Vec<Ident> = text
        .split('.')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            idents
                .iter()
                .find(|i| i.name == s)
                .cloned()
                .unwrap_or_else(|| Ident::new(s, span))
        })
        .collect();
    if segs.is_empty() {
        QualifiedName::new(vec![Ident::new(text, span)])
    } else {
        QualifiedName::new(segs)
    }
}

// =============================================================================
// Internal entries
// =============================================================================

fn convert_internal_block(node: Node, cvt: &mut Cvt) -> InternalBlock {
    let span = cvt.span(node);
    let entries: Vec<InternalEntry> = cvt
        .named_children_of(node, &["internal_entry"])
        .into_iter()
        .filter_map(|e| convert_internal_entry(e, cvt))
        .collect();
    InternalBlock { entries, span }
}

fn convert_internal_entry(node: Node, cvt: &mut Cvt) -> Option<InternalEntry> {
    let span = cvt.span(node);
    let raw = cvt.text(node).trim();
    let idents = collect_identifier_descendants(node, cvt);

    let star_brackets = raw.contains("[*]");
    let receives = raw.contains(".receives.");

    if receives {
        // Comp.receives.Msg or Comp[*].receives.Msg
        let comp_text = raw.split('[').next().unwrap_or(raw);
        let comp_text = comp_text.split(".receives.").next().unwrap_or(comp_text);
        let component = qn_from_text(comp_text, &idents, span);
        let message = idents.last().cloned().unwrap_or(Ident::new("<missing>", span));
        return Some(InternalEntry::Receives {
            component,
            all_replicas: star_brackets,
            message,
            span,
        });
    }
    if star_brackets {
        // Comp[*].action or Comp[*].*
        let comp_text = raw.split('[').next().unwrap_or(raw).trim();
        let component = qn_from_text(comp_text, &idents, span);
        if raw.ends_with(".*") {
            return Some(InternalEntry::AllOfAllReplicas { component, span });
        }
        let action = idents.last().cloned().unwrap_or(Ident::new("<missing>", span));
        return Some(InternalEntry::ActionAllReplicas { component, action, span });
    }
    if raw.ends_with(".*") {
        let comp_text = raw.trim_end_matches(".*").trim();
        let component = qn_from_text(comp_text, &idents, span);
        return Some(InternalEntry::AllOfComponent { component, span });
    }
    // Plain Comp.action
    if idents.is_empty() {
        return None;
    }
    let action = idents.last().cloned().unwrap_or(Ident::new("<missing>", span));
    let comp_text = raw.rsplit_once('.').map(|(c, _)| c).unwrap_or("");
    let component = qn_from_text(comp_text, &idents, span);
    Some(InternalEntry::Action { component, action, span })
}

// =============================================================================
// Epoch
// =============================================================================

fn convert_epoch_decl(node: Node, cvt: &mut Cvt) -> Option<EpochDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let mut advances_on: Vec<FairnessTarget> = Vec::new();
    let mut covers: Vec<Ident> = Vec::new();
    let mut cursor = node.walk();
    for c in node.children_by_field_name("advances_on", &mut cursor) {
        if c.kind() == "indexed_path" || c.kind() == "identifier" || c.kind() == "fairness_path" {
            advances_on.push(parse_fairness_target(c, cvt));
        }
    }
    let mut cursor = node.walk();
    for c in node.children_by_field_name("covers", &mut cursor) {
        if c.kind() == "identifier" {
            covers.push(ident_from_node(c, cvt));
        }
    }
    Some(EpochDecl { name, advances_on, covers, doc: None, span })
}
