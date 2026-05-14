//! Convert scenario declarations.

use super::convert_expr::{convert_call_arg, convert_expr};
use super::{ident_from_node, Cvt};
use surfacide_ast::expr::{CallArg, Expr, ExprKind};
use surfacide_ast::scenario::*;
use surfacide_ast::*;
use tree_sitter::Node;

pub fn convert_scenario_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<Scenario> {
    let span = cvt.span(node);
    let title_raw = node
        .child_by_field_name("title")
        .map(|n| cvt.text(n))
        .unwrap_or("");
    let title = title_raw
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(title_raw)
        .to_string();

    let mut kind = ScenarioKind::Safety;
    let mut tags: Vec<Ident> = Vec::new();
    let mut actors: Vec<ScenarioActor> = Vec::new();
    let mut clauses: Vec<ScenarioClause> = Vec::new();
    let mut requires_in: Vec<Ident> = Vec::new();

    for child in cvt.all_named_children(node) {
        match child.kind() {
            "scenario_kind" => {
                let val = child
                    .child_by_field_name("value")
                    .map(|n| cvt.text(n))
                    .unwrap_or("safety");
                kind = match val {
                    "liveness" => ScenarioKind::Liveness,
                    "forbidden" => ScenarioKind::Forbidden,
                    _ => ScenarioKind::Safety,
                };
            }
            "scenario_tags" => {
                let mut cursor = child.walk();
                for t in child.children_by_field_name("tag", &mut cursor) {
                    if t.kind() == "identifier" {
                        tags.push(ident_from_node(t, cvt));
                    }
                }
            }
            "scenario_actors" => {
                for a in cvt.named_children_of(child, &["scenario_actor"]) {
                    let aspan = cvt.span(a);
                    let name = a
                        .child_by_field_name("name")
                        .map(|n| ident_from_node(n, cvt))
                        .unwrap_or_else(|| Ident::new("<missing>", aspan));
                    let actor_ty = a
                        .child_by_field_name("type")
                        .map(|n| ident_from_node(n, cvt))
                        .unwrap_or_else(|| Ident::new("<missing>", aspan));
                    actors.push(ScenarioActor { name, actor_ty, span: aspan });
                }
            }
            "scenario_given" => {
                for c in cvt.all_named_children(child) {
                    let span = cvt.span(c);
                    let predicate = convert_expr(c, cvt);
                    clauses.push(ScenarioClause::Given { predicate, span });
                }
            }
            "scenario_when" => {
                for c in cvt.all_named_children(child) {
                    match c.kind() {
                        "scenario_call" => clauses.push(convert_scenario_call(c, cvt)),
                        "atomic_when" => {
                            let aspan = cvt.span(c);
                            let steps: Vec<ScenarioClause> = cvt
                                .named_children_of(c, &["scenario_call"])
                                .into_iter()
                                .map(|sc| convert_scenario_call(sc, cvt))
                                .collect();
                            clauses.push(ScenarioClause::WhenAtomic { steps, span: aspan });
                        }
                        _ => {}
                    }
                }
            }
            "scenario_then" => {
                for c in cvt.all_named_children(child) {
                    let span = cvt.span(c);
                    match c.kind() {
                        "fails_with_clause" => {
                            let error_name = c
                                .child_by_field_name("error")
                                .map(|n| ident_from_node(n, cvt))
                                .unwrap_or(Ident::new("<missing>", span));
                            clauses.push(ScenarioClause::ThenFailsWith { error_name, span });
                        }
                        "observed_clause" => {
                            clauses.push(convert_observed_clause(c, cvt, /*eventually=*/ false));
                        }
                        "eventually_observed_clause" => {
                            clauses.push(convert_observed_clause(c, cvt, /*eventually=*/ true));
                        }
                        "eventually_clause" => {
                            // Wrap the predicate in a Then; eventual liveness
                            // distinction is captured by the scenario's `kind`
                            // (liveness scenario) plus the inner predicate.
                            if let Some(inner) = cvt.all_named_children(c).into_iter().next() {
                                let predicate = convert_expr(inner, cvt);
                                clauses.push(ScenarioClause::Then { predicate, span });
                            }
                        }
                        _ => {
                            let predicate = convert_expr(c, cvt);
                            clauses.push(ScenarioClause::Then { predicate, span });
                        }
                    }
                }
            }
            "scenario_requires" => {
                let mut cursor = child.walk();
                for s in child.children_by_field_name("substrate", &mut cursor) {
                    if s.kind() == "identifier" {
                        requires_in.push(ident_from_node(s, cvt));
                    }
                }
            }
            _ => {}
        }
    }

    Some(Scenario {
        title,
        kind,
        tags,
        actors,
        clauses,
        requires_in,
        doc,
        span,
    })
}

fn convert_scenario_call(node: Node, cvt: &mut Cvt) -> ScenarioClause {
    let span = cvt.span(node);
    let actor = node
        .child_by_field_name("actor")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let action_id = node
        .child_by_field_name("action")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let action = QualifiedName::new(vec![action_id]);
    // Args are any expression children (not the actor / action identifiers).
    let actor_node = node.child_by_field_name("actor");
    let action_node = node.child_by_field_name("action");
    let args: Vec<CallArg> = cvt
        .all_named_children(node)
        .into_iter()
        .filter(|c| Some(*c) != actor_node && Some(*c) != action_node)
        .map(|c| {
            let cspan = cvt.span(c);
            CallArg { name: None, value: convert_expr(c, cvt), span: cspan }
        })
        .collect();
    ScenarioClause::When { actor, action, args, span }
}

fn convert_observed_clause(node: Node, cvt: &mut Cvt, _eventually: bool) -> ScenarioClause {
    let span = cvt.span(node);
    let event = node
        .child_by_field_name("event")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let by_actor = node
        .child_by_field_name("actor")
        .and_then(|n| {
            if n.kind() == "identifier" {
                Some(ident_from_node(n, cvt))
            } else {
                None
            }
        })
        .unwrap_or_else(|| Ident::new("_", span));
    let args: Vec<CallArg> = cvt
        .named_children_of(node, &["call_arg"])
        .into_iter()
        .map(|a| convert_call_arg(a, cvt))
        .collect();
    ScenarioClause::Observed { event, args, by_actor, span }
}
