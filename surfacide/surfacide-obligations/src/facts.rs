//! Fact extraction over the typed AST for the obligation pass.
//!
//! The §15.1.5 fact schema is normative; this module produces the
//! base facts the rules in [`crate::rules`] consume. Each fact-set
//! lives in a `HashSet` for de-dup; the obligation pass iterates the
//! sets and matches against `acknowledged { … }` entries.
//!
//! Current coverage (v0.1):
//!
//! - `channels` — every `compose` block channel (S_from, S_to).
//! - `cross_visible_owners` — substrate that declared a given aux name.
//! - `cross_visible_writers` — `(substrate, aux_name)` pairs derived
//!   from `Assign`/`AddAssign`/etc. statements whose target is an aux
//!   name owned by some substrate.
//! - `param_auth_actions` — `(surface_action, channel-tag)` pairs
//!   where the realising substrate's authentication uses `param.<x>`
//!   and the surface action's `auth_channel` slot includes one of
//!   `bearer_token | signed_request | capability_url`.
//! - `strong_freshness_with_channels` — surface actions whose
//!   `freshness: strong` (or Bounded) and whose compose realising
//!   substrate has channels (a proxy for "async path; needs sync ack").

use std::collections::{HashMap, HashSet};

use surfacide_ast::expr::{ExprKind, PathExpr};
use surfacide_ast::slot::{AuthChannelTag, FreshnessValue, SlotKind, SlotValue};
use surfacide_ast::substrate::{AuthRhs, AuxiliaryKind, SubstrateBlock};
use surfacide_ast::surface::{EffectBlock, EffectStmt, SurfaceBlock};
use surfacide_ast::compose::ComposeBlock;
use surfacide_ast::{Decl, Project};

/// All facts extracted from a single module.
#[derive(Debug, Default)]
pub struct Facts {
    /// `(compose_name, S_from, S_to)`.
    pub channels: Vec<(String, String, String)>,
    /// `aux_name -> owning_substrate_name`.
    pub cross_visible_owners: HashMap<String, String>,
    /// `(substrate, aux_name)` — the substrate where a write to this
    /// `cross_visible` aux was observed. We collect every distinct
    /// substrate that writes.
    pub cross_visible_writers: HashSet<(String, String)>,
    /// `(surface_action, trust-relevant-channels)` — actions whose
    /// auth_channel includes bearer/signed/capability AND whose
    /// realising substrate authentication uses `param.<x>`.
    pub param_auth_actions: HashSet<String>,
    /// Surface actions whose `freshness: strong` is realised through
    /// a compose channel.
    pub strong_freshness_via_channel: HashSet<String>,
    /// Surface actions whose `freshness: bounded(epoch, n=k)` is realised
    /// through a compose channel (separate set so the diagnostic wording
    /// can distinguish strong vs bounded — R9 finding).
    pub bounded_freshness_via_channel: HashSet<String>,
}

/// Extract facts from every module in the project.
pub fn extract(project: &Project) -> HashMap<String, Facts> {
    let mut out = HashMap::new();
    for (key, module) in &project.modules {
        out.insert(key.clone(), extract_module(module));
    }
    out
}

fn extract_module(module: &surfacide_ast::project::ModuleAggregate) -> Facts {
    let mut facts = Facts::default();

    // Step 1: enumerate substrates (full + partial) and surface blocks.
    let substrates: Vec<&SubstrateBlock> = module
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Substrate(s) => Some(s),
            Decl::PartialSubstrate(p) => Some(&p.block),
            _ => None,
        })
        .collect();
    let composes: Vec<&ComposeBlock> = module
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Compose(c) => Some(c),
            _ => None,
        })
        .collect();
    let surfaces: Vec<&SurfaceBlock> = module
        .decls
        .iter()
        .filter_map(|d| match d {
            Decl::Surface(s) => Some(s),
            _ => None,
        })
        .collect();

    // Step 2: channels (per compose).
    for compose in &composes {
        for ch in &compose.channels {
            if let (Some(from), Some(to)) = (
                ch.from_comp.segments.first().map(|i| i.name.clone()),
                ch.to_comp.segments.first().map(|i| i.name.clone()),
            ) {
                if from != to {
                    facts.channels.push((compose.name.name.clone(), from, to));
                }
            }
        }
    }

    // Step 3: cross_visible aux ownership.
    for sub in &substrates {
        if let Some(aux) = &sub.auxiliary {
            for var in &aux.vars {
                if var.cross_visible {
                    facts.cross_visible_owners.insert(
                        var.name.name.clone(),
                        sub.name.name.clone(),
                    );
                }
            }
        }
    }

    // Step 4: writers — scan every component action / receives handler
    // body for assignments whose target identifier matches a known
    // cross_visible aux.
    for sub in &substrates {
        let sub_name = sub.name.name.clone();
        for comp in &sub.components {
            for act in &comp.actions {
                collect_writers(&act.body, &facts.cross_visible_owners, &sub_name, &mut facts.cross_visible_writers);
            }
            for rh in &comp.receives {
                collect_writers(&rh.body, &facts.cross_visible_owners, &sub_name, &mut facts.cross_visible_writers);
            }
        }
        for rep in &sub.replicates {
            for act in &rep.component.actions {
                collect_writers(&act.body, &facts.cross_visible_owners, &sub_name, &mut facts.cross_visible_writers);
            }
            for rh in &rep.component.receives {
                collect_writers(&rh.body, &facts.cross_visible_owners, &sub_name, &mut facts.cross_visible_writers);
            }
        }
    }

    // Step 5: param-auth + trust-bearing channels.
    let bearer_like = |t: AuthChannelTag| {
        matches!(t, AuthChannelTag::BearerToken | AuthChannelTag::SignedRequest | AuthChannelTag::CapabilityUrl)
    };
    for surface in &surfaces {
        for a in surface.actions.iter().chain(surface.internal_actions.iter().map(|ia| &ia.action)) {
            let has_trust_channel = a.slots.iter().any(|s| {
                s.kind == SlotKind::AuthChannel
                    && match &s.value {
                        SlotValue::AuthChannel(v) => v.channels.iter().copied().any(bearer_like),
                        _ => false,
                    }
            });
            if !has_trust_channel {
                continue;
            }
            // Now look across substrates for a `param.<x>` authentication
            // mapping for this action.
            for sub in &substrates {
                if let Some(auth) = &sub.authentication {
                    for mapping in &auth.mappings {
                        if mapping.surface_action.last().name == a.name.name {
                            if matches!(mapping.rhs, AuthRhs::Param(_)) {
                                facts.param_auth_actions.insert(a.name.name.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 6: strong-freshness ↔ channel (R-FRESHNESS-CHANNEL).
    // Heuristic: if the action has freshness `strong` or `bounded(epoch, n)`
    // AND the module declares a compose channel, the action *may* need
    // synchronous-ack discharge. We split strong vs bounded so the
    // diagnostic can be wording-accurate.
    let module_has_channels = !facts.channels.is_empty();
    if module_has_channels {
        for surface in &surfaces {
            for a in surface.actions.iter().chain(surface.internal_actions.iter().map(|ia| &ia.action)) {
                for s in &a.slots {
                    if s.kind != SlotKind::Freshness {
                        continue;
                    }
                    match &s.value {
                        SlotValue::Freshness(FreshnessValue::Strong) => {
                            facts.strong_freshness_via_channel.insert(a.name.name.clone());
                        }
                        SlotValue::Freshness(FreshnessValue::Bounded { .. }) => {
                            facts.bounded_freshness_via_channel.insert(a.name.name.clone());
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    facts
}

fn collect_writers(
    block: &EffectBlock,
    owners: &HashMap<String, String>,
    substrate: &str,
    out: &mut HashSet<(String, String)>,
) {
    for stmt in &block.stmts {
        match stmt {
            EffectStmt::Assign { target, .. }
            | EffectStmt::AddAssign { target, .. }
            | EffectStmt::SubAssign { target, .. }
            | EffectStmt::DeleteKey { target, .. }
            | EffectStmt::SeqSnoc { target, .. } => {
                if let Some(name) = target_head_name(target) {
                    if owners.contains_key(name) {
                        out.insert((substrate.to_string(), name.to_string()));
                    }
                }
            }
            EffectStmt::IfElse { then_block, else_block, .. } => {
                collect_writers(then_block, owners, substrate, out);
                if let Some(eb) = else_block {
                    collect_writers(eb, owners, substrate, out);
                }
            }
            EffectStmt::For { body, .. } => collect_writers(body, owners, substrate, out),
            EffectStmt::Match { arms, .. } => {
                for arm in arms {
                    collect_writers(&arm.body, owners, substrate, out);
                }
            }
            EffectStmt::IfLetSome { then_block, else_block, .. } => {
                collect_writers(then_block, owners, substrate, out);
                if let Some(eb) = else_block {
                    collect_writers(eb, owners, substrate, out);
                }
            }
            _ => {}
        }
    }
}

fn target_head_name(e: &surfacide_ast::Expr) -> Option<&str> {
    match &*e.kind {
        ExprKind::Ident(id) => Some(&id.name),
        ExprKind::Path(PathExpr { head, .. }) => Some(&head.name),
        _ => None,
    }
}

// Suppress unused warning for AuxiliaryKind (kept available for future rules
// that distinguish history vs prophecy aux).
#[allow(dead_code)]
fn _force_aux_kind_import(k: AuxiliaryKind) -> AuxiliaryKind {
    k
}
