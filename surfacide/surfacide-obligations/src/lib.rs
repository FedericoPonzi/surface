//! M4 — Static-obligation derivation pass (spec §15).
//!
//! Implemented rules (v0.2):
//!
//! - **R-AVAIL-CHANNEL** (§15.4, medium). A compose channel
//!   `from S to T` derives `availability_depends_on(T)` for `S`.
//! - **R-WRITE-CONFLICT** (§15.4, high). A `cross_visible` aux variable
//!   written by two distinct substrates derives
//!   `write_conflict(<aux>)`.
//! - **R-TRUST-PARAM-AUTH** (§15.4, high). An action with
//!   `auth_channel` ∈ {bearer_token, signed_request, capability_url}
//!   and a realising substrate that uses `param.<x>` for actor
//!   identity derives `trust_transitive(<signing_authority>)`.
//! - **R-FRESHNESS-CHANNEL** (§15.4, high). A surface action with
//!   `freshness: strong` (or `bounded(…)`) realised via a compose
//!   channel derives `freshness_channel(<action>, <channel>)`.
//!
//! All derive against the same `acknowledged { … }` matching logic:
//! the ack may live in the compose block OR in any partial substrate
//! (per §15.3). Unhandled obligations emit warnings by default;
//! `--obligations=strict` promotes to `E_OBLIGATION_UNHANDLED`.
//! Orphan acknowledgements (acks with no rule-derived counterpart)
//! emit `W_ACK_NO_RULE`.

pub mod facts;

use std::collections::HashSet;

use surfacide_ast::compose::{AcknowledgedArg, AcknowledgedBlock, ObligationKind};
use surfacide_ast::{Decl, Project};
use surfacide_diag::{Diagnostic, ErrorKind, WarningKind};

#[derive(Debug, Default)]
pub struct ObligationsOutput {
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run(project: &Project, strict: bool) -> ObligationsOutput {
    let mut diagnostics = Vec::new();
    let all_facts = facts::extract(project);

    for (key, module) in &project.modules {
        let facts = match all_facts.get(key) {
            Some(f) => f,
            None => continue,
        };

        // Collect every acknowledgement (kind, name) from this module's
        // substrate-side AND compose-side `acknowledged { … }` blocks.
        let mut acked: HashSet<(ObligationKind, String)> = HashSet::new();
        for decl in &module.decls {
            match decl {
                Decl::Substrate(s) => {
                    if let Some(a) = &s.acknowledged {
                        collect_acks(a, &mut acked);
                    }
                }
                Decl::PartialSubstrate(ps) => {
                    if let Some(a) = &ps.block.acknowledged {
                        collect_acks(a, &mut acked);
                    }
                }
                Decl::Compose(c) => {
                    if let Some(a) = &c.acknowledged {
                        collect_acks(a, &mut acked);
                    }
                }
                _ => {}
            }
        }

        // Track every fact-derived (kind, arg) pair so we can detect
        // orphan acks afterwards.
        let mut derived: HashSet<(ObligationKind, String)> = HashSet::new();

        // ── R-AVAIL-CHANNEL ──
        for (compose_name, from, to) in &facts.channels {
            derived.insert((ObligationKind::AvailabilityDependsOn, to.clone()));
            let ack_key = (ObligationKind::AvailabilityDependsOn, to.clone());
            if !acked.contains(&ack_key) {
                let span = compose_span_in(module, compose_name).unwrap_or(surfacide_ast::Span::synthetic());
                diagnostics.push(make_diag(
                    strict,
                    WarningKind::AvailabilityClosureWeaker,
                    format!(
                        "compose `{}` channels depend on substrate `{}` (R-AVAIL-CHANNEL, §15.4)",
                        compose_name, to
                    ),
                    span,
                    "add `acknowledged { availability_depends_on: [<S>, because: \"...\"] }` to the compose or to either partial substrate",
                    from,
                ));
            }
        }

        // ── R-WRITE-CONFLICT ──
        let mut writers_by_aux: std::collections::HashMap<&str, HashSet<&str>> =
            std::collections::HashMap::new();
        for (sub, aux) in &facts.cross_visible_writers {
            writers_by_aux.entry(aux.as_str()).or_default().insert(sub.as_str());
        }
        for (aux, subs) in writers_by_aux {
            if subs.len() < 2 {
                continue;
            }
            derived.insert((ObligationKind::WriteConflict, aux.to_string()));
            let ack_key = (ObligationKind::WriteConflict, aux.to_string());
            // Require the ack to name *this* aux (or be unscoped/empty).
            // Naming a different aux does not discharge — self-review-2 #1.
            let unscoped_write_conflict_ack = acked
                .iter()
                .any(|(k, arg)| *k == ObligationKind::WriteConflict && arg.is_empty());
            if !acked.contains(&ack_key) && !unscoped_write_conflict_ack {
                let owner = facts.cross_visible_owners.get(aux).cloned().unwrap_or_default();
                let span = substrate_span_in(module, &owner).unwrap_or(surfacide_ast::Span::synthetic());
                let writers_list: Vec<&str> = subs.iter().copied().collect();
                let msg = format!(
                    "cross_visible aux `{}` is written by multiple substrates ({}); R-WRITE-CONFLICT (§15.4)",
                    aux,
                    writers_list.join(", ")
                );
                diagnostics.push(make_diag(
                    strict,
                    WarningKind::WriteConflict,
                    msg,
                    span,
                    "add `acknowledged { write_conflict: { <aux>: serialized_by(<Component>) because: \"...\" } }` to compose or owning substrate",
                    "",
                ));
            }
        }

        // ── R-TRUST-PARAM-AUTH ──
        //
        // Per spec §15.3 the ack just needs to *exist* with a
        // `because:` naming the signing authority. We match by kind
        // (TrustTransitive) alone — the rule is discharged when ANY
        // trust_transitive entry is present in the module. This is the
        // R9 fix for "the ack doesn't actually discharge the rule."
        let any_trust_transitive_ack = acked.iter().any(|(k, _)| *k == ObligationKind::TrustTransitive);
        for action in &facts.param_auth_actions {
            derived.insert((ObligationKind::TrustTransitive, action.clone()));
            if !any_trust_transitive_ack {
                let span = action_span_in(module, action).unwrap_or(surfacide_ast::Span::synthetic());
                diagnostics.push(make_diag(
                    strict,
                    WarningKind::TrustParamAuth,
                    format!(
                        "action `{}` carries trust-bearing auth_channel and uses `param.<x>` for actor identity; R-TRUST-PARAM-AUTH (§15.4)",
                        action
                    ),
                    span,
                    "add `acknowledged { trust_transitive: [<SigningAuthority> because: \"...\"] }` naming the key-management story (any module-level `trust_transitive` ack discharges this)",
                    action,
                ));
            }
        }

        // ── R-FRESHNESS-CHANNEL ──
        //
        // Same matching strategy: any module-level `freshness_channel`
        // ack discharges. Wording distinguishes strong-vs-bounded.
        let any_freshness_ack = acked.iter().any(|(k, _)| *k == ObligationKind::FreshnessChannel);
        for action in &facts.strong_freshness_via_channel {
            derived.insert((ObligationKind::FreshnessChannel, action.clone()));
            if !any_freshness_ack {
                let span = action_span_in(module, action).unwrap_or(surfacide_ast::Span::synthetic());
                diagnostics.push(make_diag(
                    strict,
                    WarningKind::FreshnessChannel,
                    format!(
                        "action `{}` declares `freshness: strong` but is realised via a compose channel; R-FRESHNESS-CHANNEL (§15.4)",
                        action
                    ),
                    span,
                    "either weaken `freshness:` to `eventual(<epoch>)`, add a synchronous-ack channel, or `acknowledged { freshness_channel: [...] because: \"...\" }`",
                    action,
                ));
            }
        }
        for action in &facts.bounded_freshness_via_channel {
            derived.insert((ObligationKind::FreshnessChannel, action.clone()));
            if !any_freshness_ack {
                let span = action_span_in(module, action).unwrap_or(surfacide_ast::Span::synthetic());
                diagnostics.push(make_diag(
                    strict,
                    WarningKind::FreshnessChannel,
                    format!(
                        "action `{}` declares a bounded freshness epoch but is realised via a compose channel; R-FRESHNESS-CHANNEL (§15.4)",
                        action
                    ),
                    span,
                    "either weaken `freshness:` to `eventual(<epoch>)`, widen the `n=` bound, add a synchronous-ack channel, or `acknowledged { freshness_channel: [...] because: \"...\" }`",
                    action,
                ));
            }
        }

        // ── Orphan acks (W_ACK_NO_RULE) ──
        for decl in &module.decls {
            let ack_block = match decl {
                Decl::Compose(c) => c.acknowledged.as_ref(),
                Decl::Substrate(s) => s.acknowledged.as_ref(),
                Decl::PartialSubstrate(p) => p.block.acknowledged.as_ref(),
                _ => None,
            };
            if let Some(ack) = ack_block {
                report_orphan_acks(ack, &derived, &mut diagnostics);
            }
        }
    }

    ObligationsOutput { diagnostics }
}

fn collect_acks(block: &AcknowledgedBlock, out: &mut HashSet<(ObligationKind, String)>) {
    for entry in &block.entries {
        for arg in &entry.args {
            if let Some(name) = ack_arg_name(arg) {
                out.insert((entry.kind, name));
            }
        }
    }
}

fn report_orphan_acks(
    block: &AcknowledgedBlock,
    derived: &HashSet<(ObligationKind, String)>,
    out: &mut Vec<Diagnostic>,
) {
    // Per the R9 critique: an ack of kind K is "discharging" any rule
    // that derives an obligation of kind K in the same module. So a
    // proactive ack is orphan only when *no* derivation of that kind
    // fired in this module at all.
    let derived_kinds: HashSet<ObligationKind> = derived.iter().map(|(k, _)| *k).collect();
    for entry in &block.entries {
        // Whole-kind discharge: if any obligation of this entry's kind
        // was derived in the module, no orphan warning.
        if derived_kinds.contains(&entry.kind) {
            continue;
        }
        for arg in &entry.args {
            if let Some(name) = ack_arg_name(arg) {
                out.push(
                    Diagnostic::warning(
                        WarningKind::AckNoRule,
                        format!(
                            "acknowledged {}(`{}`), but no v0.10 rule derives this obligation in this module",
                            entry.kind.name(),
                            name
                        ),
                        entry.span,
                    )
                    .with_help(
                        "this acknowledgement is recorded but not mechanically discharged; \
                         consider opening a TODO for a future rule, or remove if intentional",
                    ),
                );
            }
        }
    }
}

fn ack_arg_name(arg: &AcknowledgedArg) -> Option<String> {
    match arg {
        AcknowledgedArg::Component(qn) => Some(qn.segments.first()?.name.clone()),
        AcknowledgedArg::Field(id) | AcknowledgedArg::Action(id) => Some(id.name.clone()),
        AcknowledgedArg::Other(_) => None,
    }
}

fn make_diag(
    strict: bool,
    medium_warning: WarningKind,
    msg: String,
    span: surfacide_ast::Span,
    help: &str,
    _ack_target: &str,
) -> Diagnostic {
    if strict {
        Diagnostic::error(ErrorKind::ObligationUnhandled, msg, span).with_help(help)
    } else {
        Diagnostic::warning(medium_warning, msg, span).with_help(help)
    }
}

fn compose_span_in(
    module: &surfacide_ast::project::ModuleAggregate,
    compose_name: &str,
) -> Option<surfacide_ast::Span> {
    module.decls.iter().find_map(|d| match d {
        Decl::Compose(c) if c.name.name == compose_name => Some(c.span),
        _ => None,
    })
}

fn substrate_span_in(
    module: &surfacide_ast::project::ModuleAggregate,
    name: &str,
) -> Option<surfacide_ast::Span> {
    module.decls.iter().find_map(|d| match d {
        Decl::Substrate(s) if s.name.name == name => Some(s.span),
        Decl::PartialSubstrate(p) if p.block.name.name == name => Some(p.block.span),
        _ => None,
    })
}

fn action_span_in(
    module: &surfacide_ast::project::ModuleAggregate,
    action_name: &str,
) -> Option<surfacide_ast::Span> {
    for d in &module.decls {
        if let Decl::Surface(s) = d {
            for a in &s.actions {
                if a.name.name == action_name {
                    return Some(a.name.span);
                }
            }
            for ia in &s.internal_actions {
                if ia.action.name.name == action_name {
                    return Some(ia.action.name.span);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use surfacide_ast::*;
    use surfacide_ast::compose::*;
    use surfacide_ast::substrate::*;
    use surfacide_ast::project::ModuleAggregate;

    fn id(s: &str) -> Ident {
        Ident::new(s, Span::synthetic())
    }

    fn channel(name: &str, from: &str, to: &str) -> ChannelDecl {
        ChannelDecl {
            name: id(name),
            from_comp: QualifiedName::new(vec![id(from)]),
            from_mult: ChannelMultiplicity::One,
            to_comp: QualifiedName::new(vec![id(to)]),
            to_mult: ChannelMultiplicity::One,
            span: Span::synthetic(),
        }
    }

    fn compose_with(
        name: &str,
        channels: Vec<ChannelDecl>,
        ack: Option<AcknowledgedBlock>,
    ) -> ComposeBlock {
        ComposeBlock {
            name: id(name),
            members: Vec::new(),
            channels,
            realizes: None,
            acknowledged: ack,
            fairness: Vec::new(),
            doc: None,
            span: Span::synthetic(),
        }
    }

    fn make_project(c: ComposeBlock) -> Project {
        let mut p = Project::new();
        p.modules.insert(
            "M".to_string(),
            ModuleAggregate {
                name: QualifiedName::new(vec![id("M")]),
                private: false,
                files: Vec::new(),
                decls: vec![Decl::Compose(c)],
            },
        );
        p
    }

    fn ack_entry(kind: ObligationKind, name: &str) -> AcknowledgedEntry {
        AcknowledgedEntry {
            kind,
            args: vec![AcknowledgedArg::Component(QualifiedName::new(vec![id(name)]))],
            resolution: None,
            because: Some("test".to_string()),
            because_span: Some(Span::synthetic()),
            span: Span::synthetic(),
        }
    }

    #[test]
    fn channel_without_acknowledgement_warns() {
        let p = make_project(compose_with("Prod", vec![channel("c", "S1", "S2")], None));
        let out = run(&p, false);
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(out.diagnostics[0].code.as_str(), "W_AVAILABILITY_CLOSURE_WEAKER");
    }

    #[test]
    fn channel_with_acknowledgement_is_clean() {
        let p = make_project(compose_with(
            "Prod",
            vec![channel("c", "S1", "S2")],
            Some(AcknowledgedBlock {
                entries: vec![ack_entry(ObligationKind::AvailabilityDependsOn, "S2")],
                span: Span::synthetic(),
            }),
        ));
        let out = run(&p, false);
        assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
    }

    #[test]
    fn orphan_acknowledgement_warns() {
        let p = make_project(compose_with(
            "Prod",
            Vec::new(),
            Some(AcknowledgedBlock {
                entries: vec![ack_entry(ObligationKind::AvailabilityDependsOn, "Mystery")],
                span: Span::synthetic(),
            }),
        ));
        let out = run(&p, false);
        assert!(out.diagnostics.iter().any(|d| d.code.as_str() == "W_ACK_NO_RULE"));
    }

    /// Self-review must-fix #5: strict mode must emit a proper
    /// obligation-unhandled error code, not `E_INTERNAL`.
    #[test]
    fn strict_mode_promotes_to_error() {
        let p = make_project(compose_with("Prod", vec![channel("c", "S1", "S2")], None));
        let out = run(&p, true);
        let err = out
            .diagnostics
            .iter()
            .find(|d| d.is_error())
            .expect("strict mode must error");
        assert_eq!(err.code.as_str(), "E_OBLIGATION_UNHANDLED");
    }
}

