//! M3 — Mandatory action slot pass (spec §6.4).
//!
//! For every surface (or `internal_action`) action in the project:
//!
//! 1. **Source ordering** of slot lines must match the canonical
//!    order (§6.4: `idempotency, auth_channel, retention, rate_limit,
//!    observability, availability, freshness`). Out of order →
//!    `E_SURFACE_SLOT_ORDER`.
//! 2. After elaboration (defaults + internal_action presets, §6.4.5/6
//!    with v0.10.1 precedence in §6.4.6.1), every action carries an
//!    effective value for **all seven** slots. Missing →
//!    `E_SURFACE_SLOT_MISSING`.
//! 3. Slot values are well-typed against the closed enum
//!    (parsed-as-Unknown → `E_SURFACE_SLOT_UNKNOWN_VALUE`).
//! 4. Waiver reasons must be non-empty
//!    (`E_SURFACE_SLOT_WAIVER_EMPTY`).
//!
//! Cross-slot consistency checks (auth_channel↔authentication,
//! retention-secret flow into emit, derived-field assignment) live in
//! [`crate::cross_slot`] (different pass entry point but same diag
//! buffer at the CLI).

use indexmap::IndexMap;
use surfacide_ast::{Decl, Project, Span};
use surfacide_ast::slot::{SlotKind, SlotProvenance, SlotValue};
use surfacide_ast::surface::{ActionDecl, DefaultsBlock, SurfaceBlock};
use surfacide_diag::{Diagnostic, ErrorKind};

/// One slot's elaborated value, with the source provenance preserved.
#[derive(Debug, Clone)]
pub struct EffectiveSlot {
    pub kind: SlotKind,
    pub value: SlotValue,
    pub provenance: SlotProvenance,
    /// Where the value was *declared*. For provenance::Default it's the
    /// defaults-block entry; for InternalActionPreset it's the action
    /// span (the keyword); for Explicit it's the slot-assign's span.
    pub source_span: Span,
}

/// Per-action elaboration result. Always seven entries (one per
/// `SlotKind` in canonical order). Entries may be `None` when a slot
/// was neither set explicitly, defaulted, nor preset — in which case
/// the slot pass emits `E_SURFACE_SLOT_MISSING`.
#[derive(Debug, Default)]
pub struct ElaboratedSlots {
    pub idempotency: Option<EffectiveSlot>,
    pub auth_channel: Option<EffectiveSlot>,
    pub retention: Option<EffectiveSlot>,
    pub rate_limit: Option<EffectiveSlot>,
    pub observability: Option<EffectiveSlot>,
    pub availability: Option<EffectiveSlot>,
    pub freshness: Option<EffectiveSlot>,
}

impl ElaboratedSlots {
    /// Return `(kind, slot)` for every present effective slot, in
    /// canonical order.
    pub fn iter(&self) -> impl Iterator<Item = (SlotKind, &EffectiveSlot)> {
        SlotKind::canonical_order()
            .into_iter()
            .filter_map(move |k| self.get(k).map(move |s| (k, s)))
    }

    fn slot_mut(&mut self, k: SlotKind) -> &mut Option<EffectiveSlot> {
        match k {
            SlotKind::Idempotency => &mut self.idempotency,
            SlotKind::AuthChannel => &mut self.auth_channel,
            SlotKind::Retention => &mut self.retention,
            SlotKind::RateLimit => &mut self.rate_limit,
            SlotKind::Observability => &mut self.observability,
            SlotKind::Availability => &mut self.availability,
            SlotKind::Freshness => &mut self.freshness,
        }
    }

    pub fn get(&self, k: SlotKind) -> Option<&EffectiveSlot> {
        match k {
            SlotKind::Idempotency => self.idempotency.as_ref(),
            SlotKind::AuthChannel => self.auth_channel.as_ref(),
            SlotKind::Retention => self.retention.as_ref(),
            SlotKind::RateLimit => self.rate_limit.as_ref(),
            SlotKind::Observability => self.observability.as_ref(),
            SlotKind::Availability => self.availability.as_ref(),
            SlotKind::Freshness => self.freshness.as_ref(),
        }
    }
}

/// Top-level slot pass entry point. Walks every module's surface block
/// and elaborates + checks every action.
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
    if let Some(d) = &surface.defaults {
        check_defaults_uniqueness(d, out);
    }
    for a in &surface.actions {
        check_action(a, surface.defaults.as_ref(), out);
    }
    for ia in &surface.internal_actions {
        check_action(&ia.action, surface.defaults.as_ref(), out);
    }
}

/// Public for testing — returns the elaborated slot table alongside any
/// diagnostics produced.
pub fn elaborate(
    action: &ActionDecl,
    defaults: Option<&DefaultsBlock>,
) -> (ElaboratedSlots, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let mut elab = ElaboratedSlots::default();

    // (1) Source-order check on the raw `action.slots`.
    check_canonical_order(action, &mut diags);

    // (2) internal_action presets (lowest priority).
    if action.is_internal {
        apply_internal_action_presets(action, &mut elab);
    }

    // (3) defaults overlay (overrides internal_action presets).
    if let Some(d) = defaults {
        apply_defaults(d, &mut elab);
    }

    // (4) Per-action slots (highest priority).
    apply_per_action(action, &mut elab, &mut diags);

    // (5) Validate each effective value.
    for (kind, slot) in elab.iter() {
        if let SlotValue::Unknown(id) = &slot.value {
            diags.push(
                Diagnostic::error(
                    ErrorKind::SurfaceSlotUnknownValue,
                    format!(
                        "unknown value `{}` for slot `{}` on action `{}`",
                        id.name,
                        kind.name(),
                        action.name.name
                    ),
                    id.span,
                )
                .with_help(legal_values_help(kind)),
            );
        }
        if let SlotValue::Waived { reason, reason_span } = &slot.value {
            if reason.trim().is_empty() {
                diags.push(Diagnostic::error(
                    ErrorKind::SurfaceSlotWaiverEmpty,
                    format!(
                        "waiver of slot `{}` on action `{}` has empty reason",
                        kind.name(),
                        action.name.name
                    ),
                    *reason_span,
                ));
            }
        }
    }

    // (6) Missing-slot check (after full elaboration).
    for k in SlotKind::canonical_order() {
        if elab.get(k).is_none() {
            diags.push(
                Diagnostic::error(
                    ErrorKind::SurfaceSlotMissing,
                    format!(
                        "action `{}` is missing required slot `{}`",
                        action.name.name,
                        k.name()
                    ),
                    action.name.span,
                )
                .with_help(missing_slot_help(k)),
            );
        }
    }

    (elab, diags)
}

fn check_action(
    action: &ActionDecl,
    defaults: Option<&DefaultsBlock>,
    out: &mut Vec<Diagnostic>,
) {
    let (_elab, diags) = elaborate(action, defaults);
    out.extend(diags);
}

fn check_canonical_order(action: &ActionDecl, out: &mut Vec<Diagnostic>) {
    let order = SlotKind::canonical_order();
    let mut seen_indices: IndexMap<SlotKind, Span> = IndexMap::new();
    let mut last_rank: Option<(usize, SlotKind)> = None;
    for slot in &action.slots {
        let rank = order.iter().position(|k| *k == slot.kind).unwrap_or(usize::MAX);
        if let Some((last_rank_idx, last_kind)) = last_rank {
            if rank < last_rank_idx {
                let _ = seen_indices.entry(slot.kind).or_insert(slot.span);
                out.push(
                    Diagnostic::error(
                        ErrorKind::SurfaceSlotOrder,
                        format!(
                            "slot `{}` appears after `{}`; canonical order is {}",
                            slot.kind.name(),
                            last_kind.name(),
                            order
                                .iter()
                                .map(|k| k.name())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        slot.span,
                    )
                    .with_help(format!(
                        "move `{}` to its canonical position",
                        slot.kind.name()
                    )),
                );
            }
        }
        last_rank = Some((rank, slot.kind));
    }
}

fn apply_internal_action_presets(action: &ActionDecl, elab: &mut ElaboratedSlots) {
    // Per §6.4.6: internal_action auto-fills three slots:
    //   auth_channel: trusted_caller
    //   rate_limit:   waived: "internal"
    //   availability: maintenance_window
    let span = action.name.span;
    *elab.slot_mut(SlotKind::AuthChannel) = Some(EffectiveSlot {
        kind: SlotKind::AuthChannel,
        value: SlotValue::AuthChannel(surfacide_ast::slot::AuthChannelValue {
            channels: vec![surfacide_ast::slot::AuthChannelTag::TrustedCaller],
            set_form: false,
        }),
        provenance: SlotProvenance::InternalActionPreset,
        source_span: span,
    });
    *elab.slot_mut(SlotKind::RateLimit) = Some(EffectiveSlot {
        kind: SlotKind::RateLimit,
        value: SlotValue::Waived {
            reason: "internal".to_string(),
            reason_span: span,
        },
        provenance: SlotProvenance::InternalActionPreset,
        source_span: span,
    });
    *elab.slot_mut(SlotKind::Availability) = Some(EffectiveSlot {
        kind: SlotKind::Availability,
        value: SlotValue::Availability(surfacide_ast::slot::AvailabilityValue::MaintenanceWindow),
        provenance: SlotProvenance::InternalActionPreset,
        source_span: span,
    });
}

fn apply_defaults(defaults: &DefaultsBlock, elab: &mut ElaboratedSlots) {
    // Caller (`check_surface`) is responsible for emitting
    // `E_SLOT_PRECEDENCE_AMBIGUOUS` once per defaults block when a
    // slot is set twice. Here we just take the last write.
    for sa in &defaults.slots {
        *elab.slot_mut(sa.kind) = Some(EffectiveSlot {
            kind: sa.kind,
            value: sa.value.clone(),
            provenance: SlotProvenance::Default,
            source_span: sa.span,
        });
    }
}

/// Emit one `E_SLOT_PRECEDENCE_AMBIGUOUS` per duplicate key in the
/// defaults block. Called once per surface, not per action.
fn check_defaults_uniqueness(defaults: &DefaultsBlock, out: &mut Vec<Diagnostic>) {
    let mut seen: IndexMap<SlotKind, Span> = IndexMap::new();
    for sa in &defaults.slots {
        if let Some(prev) = seen.insert(sa.kind, sa.span) {
            out.push(
                Diagnostic::error(
                    ErrorKind::SlotPrecedenceAmbiguous,
                    format!(
                        "slot `{}` is set twice in `defaults`; the effective value is ambiguous (spec §6.4.6.1)",
                        sa.kind.name()
                    ),
                    sa.span,
                )
                .with_label(prev, "previous default here")
                .with_help("keep one default; per-action overrides may still set this slot explicitly"),
            );
        }
    }
}

fn apply_per_action(action: &ActionDecl, elab: &mut ElaboratedSlots, out: &mut Vec<Diagnostic>) {
    let mut seen: IndexMap<SlotKind, Span> = IndexMap::new();
    for sa in &action.slots {
        if let Some(prev_span) = seen.insert(sa.kind, sa.span) {
            // Duplicate within a single action — treat as Unknown so
            // the value pass complains about the second occurrence
            // making no sense. Emit a specific diagnostic too.
            out.push(
                Diagnostic::error(
                    ErrorKind::SurfaceSlotOrder,
                    format!(
                        "slot `{}` declared twice on action `{}`",
                        sa.kind.name(),
                        action.name.name
                    ),
                    sa.span,
                )
                .with_label(prev_span, "previous declaration here"),
            );
        }
        *elab.slot_mut(sa.kind) = Some(EffectiveSlot {
            kind: sa.kind,
            value: sa.value.clone(),
            provenance: SlotProvenance::Explicit,
            source_span: sa.span,
        });
    }
}

fn legal_values_help(kind: SlotKind) -> &'static str {
    match kind {
        SlotKind::Idempotency => {
            "legal: `idempotent`, `idempotent by(<args>)`, `at_most_once`, `at_least_once`, `waived: \"<reason>\"`"
        }
        SlotKind::AuthChannel => {
            "legal: `session`, `bearer_token`, `signed_request`, `capability_url`, `mtls`, `trusted_caller`, `anonymous`, a set `{c1, c2, …}`, or `waived: \"<reason>\"`"
        }
        SlotKind::Retention => {
            "legal: `ephemeral`, `transactional`, `audit(period=<D>)`, `pii(class=<C>, ttl=<D>)`, `secret`, `waived: \"<reason>\"`"
        }
        SlotKind::RateLimit => {
            "legal: `per_actor(n, per=<D>)`, `per_target(arg, n, per=<D>)`, `global(n, per=<D>)`, `unlimited`, `waived: \"<reason>\"`"
        }
        SlotKind::Observability => {
            "legal: `caller_only(E, …)`, `target(arg, E, …)`, `broadcast(E, …)`, `silent`, `waived: \"<reason>\"`"
        }
        SlotKind::Availability => {
            "legal: `critical`, `best_effort`, `maintenance_window`, `read_only_failover`, `waived: \"<reason>\"`"
        }
        SlotKind::Freshness => {
            "legal: `strong`, `bounded(<epoch>, n=<k>)`, `eventual(<epoch>?)`, `stale_while_revalidate(<epoch>, n=<k>)`, `not_applicable`, `waived: \"<reason>\"`"
        }
    }
}

fn missing_slot_help(kind: SlotKind) -> &'static str {
    match kind {
        SlotKind::Idempotency => "add `idempotency: <value>` or set a project-wide default",
        SlotKind::AuthChannel => "add `auth_channel: <value>` (or `internal_action` if this is an internal one)",
        SlotKind::Retention => "add `retention: <value>` or default it in `defaults { … }`",
        SlotKind::RateLimit => "add `rate_limit: <value>` (or `internal_action` for control-plane actions)",
        SlotKind::Observability => "add `observability: <value>` referencing the events your `then` block emits",
        SlotKind::Availability => "add `availability: <value>` (or `internal_action` for ops actions)",
        SlotKind::Freshness => "add `freshness: <value>` referencing a substrate-declared epoch (or `strong`/`not_applicable`)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use surfacide_ast::*;
    use surfacide_ast::slot::*;
    use surfacide_ast::surface::*;
    use surfacide_ast::decl::ActorBinder;

    fn id(name: &str) -> Ident {
        Ident::new(name, Span::synthetic())
    }

    fn empty_action(name: &str, is_internal: bool) -> ActionDecl {
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
            is_internal,
        }
    }

    fn explicit(kind: SlotKind, value: SlotValue) -> SlotAssign {
        SlotAssign { kind, value, span: Span::synthetic() }
    }

    #[test]
    fn empty_action_misses_all_seven_slots() {
        let a = empty_action("foo", false);
        let (_elab, diags) = elaborate(&a, None);
        let missing: Vec<_> = diags
            .iter()
            .filter(|d| d.code.as_str() == "E_SURFACE_SLOT_MISSING")
            .collect();
        assert_eq!(missing.len(), 7);
    }

    #[test]
    fn internal_action_auto_fills_three_still_misses_four() {
        let a = empty_action("foo", true);
        let (elab, diags) = elaborate(&a, None);
        assert_eq!(elab.auth_channel.as_ref().unwrap().provenance, SlotProvenance::InternalActionPreset);
        assert_eq!(elab.rate_limit.as_ref().unwrap().provenance, SlotProvenance::InternalActionPreset);
        assert_eq!(elab.availability.as_ref().unwrap().provenance, SlotProvenance::InternalActionPreset);
        let missing: Vec<_> = diags
            .iter()
            .filter(|d| d.code.as_str() == "E_SURFACE_SLOT_MISSING")
            .collect();
        assert_eq!(missing.len(), 4, "expected 4 missing, got {:?}", missing.iter().map(|d| d.message.clone()).collect::<Vec<_>>());
    }

    #[test]
    fn per_action_overrides_default_overrides_internal_preset() {
        // Setup: an internal_action; defaults sets auth_channel to
        // session; per-action override to anonymous. Effective should
        // be Explicit / anonymous.
        let mut a = empty_action("foo", true);
        a.slots.push(explicit(
            SlotKind::AuthChannel,
            SlotValue::AuthChannel(AuthChannelValue {
                channels: vec![AuthChannelTag::Anonymous],
                set_form: false,
            }),
        ));
        let defaults = DefaultsBlock {
            slots: vec![explicit(
                SlotKind::AuthChannel,
                SlotValue::AuthChannel(AuthChannelValue {
                    channels: vec![AuthChannelTag::Session],
                    set_form: false,
                }),
            )],
            span: Span::synthetic(),
        };
        let (elab, _diags) = elaborate(&a, Some(&defaults));
        let auth = elab.auth_channel.as_ref().unwrap();
        assert_eq!(auth.provenance, SlotProvenance::Explicit);
        if let SlotValue::AuthChannel(v) = &auth.value {
            assert_eq!(v.channels, vec![AuthChannelTag::Anonymous]);
        } else {
            panic!("expected AuthChannel value");
        }
    }

    #[test]
    fn empty_waiver_is_diagnosed() {
        let mut a = empty_action("foo", false);
        for k in SlotKind::canonical_order() {
            a.slots.push(explicit(
                k,
                SlotValue::Waived {
                    reason: "".to_string(),
                    reason_span: Span::synthetic(),
                },
            ));
        }
        let (_elab, diags) = elaborate(&a, None);
        assert_eq!(
            diags
                .iter()
                .filter(|d| d.code.as_str() == "E_SURFACE_SLOT_WAIVER_EMPTY")
                .count(),
            7
        );
    }

    #[test]
    fn out_of_order_slots_diagnosed() {
        // Place auth_channel BEFORE idempotency (reversed canonical
        // order) — should emit E_SURFACE_SLOT_ORDER.
        let mut a = empty_action("foo", false);
        a.slots.push(explicit(
            SlotKind::AuthChannel,
            SlotValue::AuthChannel(AuthChannelValue {
                channels: vec![AuthChannelTag::Session],
                set_form: false,
            }),
        ));
        a.slots.push(explicit(
            SlotKind::Idempotency,
            SlotValue::Idempotency(IdempotencyValue::Idempotent),
        ));
        let (_elab, diags) = elaborate(&a, None);
        assert!(diags
            .iter()
            .any(|d| d.code.as_str() == "E_SURFACE_SLOT_ORDER"));
    }

    #[test]
    fn unknown_slot_value_diagnosed() {
        let mut a = empty_action("foo", false);
        a.slots.push(explicit(
            SlotKind::AuthChannel,
            SlotValue::Unknown(id("dragonsbreath")),
        ));
        let (_elab, diags) = elaborate(&a, None);
        assert!(diags
            .iter()
            .any(|d| d.code.as_str() == "E_SURFACE_SLOT_UNKNOWN_VALUE"));
    }

    #[test]
    fn duplicate_slot_diagnosed() {
        let mut a = empty_action("foo", false);
        a.slots.push(explicit(
            SlotKind::Idempotency,
            SlotValue::Idempotency(IdempotencyValue::Idempotent),
        ));
        a.slots.push(explicit(
            SlotKind::Idempotency,
            SlotValue::Idempotency(IdempotencyValue::AtMostOnce),
        ));
        let (_elab, diags) = elaborate(&a, None);
        assert!(diags
            .iter()
            .any(|d| d.code.as_str() == "E_SURFACE_SLOT_ORDER"));
    }

    #[test]
    fn fully_populated_action_passes() {
        let mut a = empty_action("foo", false);
        for k in SlotKind::canonical_order() {
            let v = match k {
                SlotKind::Idempotency => SlotValue::Idempotency(IdempotencyValue::Idempotent),
                SlotKind::AuthChannel => SlotValue::AuthChannel(AuthChannelValue {
                    channels: vec![AuthChannelTag::Session],
                    set_form: false,
                }),
                SlotKind::Retention => SlotValue::Retention(RetentionValue::Ephemeral),
                SlotKind::RateLimit => SlotValue::RateLimit(RateLimitValue::Unlimited),
                SlotKind::Observability => {
                    SlotValue::Observability(ObservabilityValue::Silent)
                }
                SlotKind::Availability => {
                    SlotValue::Availability(AvailabilityValue::Critical)
                }
                SlotKind::Freshness => SlotValue::Freshness(FreshnessValue::Strong),
            };
            a.slots.push(explicit(k, v));
        }
        let (elab, diags) = elaborate(&a, None);
        assert!(diags.is_empty(), "expected clean, got {:#?}", diags);
        assert!(elab.iter().count() == 7);
    }

    #[test]
    fn defaults_override_internal_action_preset() {
        // internal_action presets `availability: maintenance_window`;
        // defaults asks for `critical`; effective should be Default /
        // critical (defaults > internal-preset per §6.4.6.1).
        let a = empty_action("foo", true);
        let defaults = DefaultsBlock {
            slots: vec![explicit(
                SlotKind::Availability,
                SlotValue::Availability(AvailabilityValue::Critical),
            )],
            span: Span::synthetic(),
        };
        let (elab, _diags) = elaborate(&a, Some(&defaults));
        let av = elab.availability.as_ref().unwrap();
        assert_eq!(av.provenance, SlotProvenance::Default);
        assert!(matches!(av.value, SlotValue::Availability(AvailabilityValue::Critical)));
    }

    #[test]
    fn duplicate_in_defaults_fires_precedence_ambiguous_once_per_surface() {
        // Two actions share a defaults block with a duplicate
        // `auth_channel` entry — error should fire once, not twice.
        let a = empty_action("foo", false);
        let b = empty_action("bar", false);
        let surface = SurfaceBlock {
            state: Vec::new(),
            init: Vec::new(),
            fairness: Vec::new(),
            properties: Vec::new(),
            defaults: Some(DefaultsBlock {
                slots: vec![
                    explicit(
                        SlotKind::AuthChannel,
                        SlotValue::AuthChannel(AuthChannelValue {
                            channels: vec![AuthChannelTag::Session],
                            set_form: false,
                        }),
                    ),
                    explicit(
                        SlotKind::AuthChannel,
                        SlotValue::AuthChannel(AuthChannelValue {
                            channels: vec![AuthChannelTag::Anonymous],
                            set_form: false,
                        }),
                    ),
                ],
                span: Span::synthetic(),
            }),
            actions: vec![a, b],
            internal_actions: Vec::new(),
            observables: Vec::new(),
            span: Span::synthetic(),
            doc: None,
        };
        let mut diags = Vec::new();
        check_surface(&surface, &mut diags);
        let ambig: Vec<_> = diags
            .iter()
            .filter(|d| d.code.as_str() == "E_SLOT_PRECEDENCE_AMBIGUOUS")
            .collect();
        assert_eq!(ambig.len(), 1, "expected exactly one E_SLOT_PRECEDENCE_AMBIGUOUS, got {}: {:#?}", ambig.len(), diags);
    }
}
