//! Stable diagnostic codes.
//!
//! Every error and warning that Surfacide can emit has a code in this
//! file. The codes are part of the **public CLI surface** — the
//! `trycmd` integration tests assert on exact code strings — so adding,
//! renaming, or removing a code is a breaking change.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    Error(ErrorKind),
    Warning(WarningKind),
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Error(e) => e.as_str(),
            Code::Warning(w) => w.as_str(),
        }
    }
}

/// Error codes. All start with `E_`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    // Parsing
    ParseError,

    // Name resolution
    NameNotFound,
    NameAmbiguous,
    PrivateModuleAccess,

    // Slot pass (§6.4)
    SurfaceSlotMissing,
    SurfaceSlotUnknownValue,
    SurfaceSlotOrder,
    SurfaceSlotWaiverEmpty,
    SlotPrecedenceAmbiguous,

    // Derived state (§6.6)
    DerivedAssign,
    DerivedNoProjection,

    // Retention / secret flow (§6.5)
    SecretFlow,

    // Freshness (§6.4.1, §7.2.4)
    FreshnessUndeclaredEpoch,

    // Actor-relative observables (§5.3)
    ActorViewLeak,

    // Acknowledged-obligation pass (§15)
    AckDisagreement,
    /// A medium/high-severity obligation derived by the §15 catalog
    /// was not acknowledged and `--obligations=strict` is in effect.
    ObligationUnhandled,

    // Module
    DuplicateSurfaceBlock,
    DuplicateActionName,

    // Internal compiler errors (should not happen but reportable)
    Internal,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::ParseError => "E_PARSE",
            ErrorKind::NameNotFound => "E_NAME_NOT_FOUND",
            ErrorKind::NameAmbiguous => "E_NAME_AMBIGUOUS",
            ErrorKind::PrivateModuleAccess => "E_PRIVATE_MODULE_ACCESS",
            ErrorKind::SurfaceSlotMissing => "E_SURFACE_SLOT_MISSING",
            ErrorKind::SurfaceSlotUnknownValue => "E_SURFACE_SLOT_UNKNOWN_VALUE",
            ErrorKind::SurfaceSlotOrder => "E_SURFACE_SLOT_ORDER",
            ErrorKind::SurfaceSlotWaiverEmpty => "E_SURFACE_SLOT_WAIVER_EMPTY",
            ErrorKind::SlotPrecedenceAmbiguous => "E_SLOT_PRECEDENCE_AMBIGUOUS",
            ErrorKind::DerivedAssign => "E_DERIVED_ASSIGN",
            ErrorKind::DerivedNoProjection => "E_DERIVED_NO_PROJECTION",
            ErrorKind::SecretFlow => "E_SECRET_FLOW",
            ErrorKind::FreshnessUndeclaredEpoch => "E_FRESHNESS_UNDECLARED_EPOCH",
            ErrorKind::ActorViewLeak => "E_ACTOR_VIEW_LEAK",
            ErrorKind::AckDisagreement => "E_ACK_DISAGREEMENT",
            ErrorKind::ObligationUnhandled => "E_OBLIGATION_UNHANDLED",
            ErrorKind::DuplicateSurfaceBlock => "E_DUPLICATE_SURFACE_BLOCK",
            ErrorKind::DuplicateActionName => "E_DUPLICATE_ACTION_NAME",
            ErrorKind::Internal => "E_INTERNAL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarningKind {
    /// Proactive acknowledgement; v0.10 downgraded from error.
    AckNoRule,
    /// Branch-coverage warning for unlabelled multi-disjunct guards.
    BranchUnlabelled,
    /// `eventually` without backing fairness.
    LivenessNoFairness,
    /// R-AVAIL-CHANNEL / R-AVAIL-READ medium.
    AvailabilityClosureWeaker,
    /// R-TRUST-PARAM-AUTH (high, but emitted as warning by default).
    TrustParamAuth,
    /// R-FRESHNESS-CHANNEL (high, but emitted as warning by default).
    FreshnessChannel,
    /// R-WRITE-CONFLICT (high, but emitted as warning by default).
    WriteConflict,
}

impl WarningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WarningKind::AckNoRule => "W_ACK_NO_RULE",
            WarningKind::BranchUnlabelled => "W_BRANCH_UNLABELLED",
            WarningKind::LivenessNoFairness => "W_LIVENESS_NO_FAIRNESS",
            WarningKind::AvailabilityClosureWeaker => "W_AVAILABILITY_CLOSURE_WEAKER",
            WarningKind::TrustParamAuth => "W_TRUST_PARAM_AUTH",
            WarningKind::FreshnessChannel => "W_FRESHNESS_CHANNEL",
            WarningKind::WriteConflict => "W_WRITE_CONFLICT",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_error_code_starts_with_e() {
        for c in [
            ErrorKind::ParseError,
            ErrorKind::NameNotFound,
            ErrorKind::SurfaceSlotMissing,
            ErrorKind::SurfaceSlotUnknownValue,
            ErrorKind::SurfaceSlotOrder,
            ErrorKind::SurfaceSlotWaiverEmpty,
            ErrorKind::SlotPrecedenceAmbiguous,
            ErrorKind::DerivedAssign,
            ErrorKind::DerivedNoProjection,
            ErrorKind::SecretFlow,
            ErrorKind::FreshnessUndeclaredEpoch,
            ErrorKind::ActorViewLeak,
            ErrorKind::AckDisagreement,
            ErrorKind::ObligationUnhandled,
            ErrorKind::DuplicateSurfaceBlock,
            ErrorKind::DuplicateActionName,
            ErrorKind::Internal,
            ErrorKind::PrivateModuleAccess,
            ErrorKind::NameAmbiguous,
        ] {
            assert!(c.as_str().starts_with("E_"), "{}", c.as_str());
        }
    }

    #[test]
    fn every_warning_code_starts_with_w() {
        for w in [
            WarningKind::AckNoRule,
            WarningKind::BranchUnlabelled,
            WarningKind::LivenessNoFairness,
            WarningKind::AvailabilityClosureWeaker,
            WarningKind::TrustParamAuth,
            WarningKind::FreshnessChannel,
            WarningKind::WriteConflict,
        ] {
            assert!(w.as_str().starts_with("W_"), "{}", w.as_str());
        }
    }

    #[test]
    fn code_strings_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for s in [
            "E_PARSE", "E_NAME_NOT_FOUND", "E_NAME_AMBIGUOUS",
            "E_PRIVATE_MODULE_ACCESS",
            "E_SURFACE_SLOT_MISSING", "E_SURFACE_SLOT_UNKNOWN_VALUE",
            "E_SURFACE_SLOT_ORDER", "E_SURFACE_SLOT_WAIVER_EMPTY",
            "E_SLOT_PRECEDENCE_AMBIGUOUS",
            "E_DERIVED_ASSIGN", "E_DERIVED_NO_PROJECTION", "E_SECRET_FLOW",
            "E_FRESHNESS_UNDECLARED_EPOCH", "E_ACTOR_VIEW_LEAK",
            "E_ACK_DISAGREEMENT", "E_OBLIGATION_UNHANDLED",
            "E_DUPLICATE_SURFACE_BLOCK",
            "E_DUPLICATE_ACTION_NAME", "E_INTERNAL",
            "W_ACK_NO_RULE", "W_BRANCH_UNLABELLED",
            "W_LIVENESS_NO_FAIRNESS", "W_AVAILABILITY_CLOSURE_WEAKER",
            "W_TRUST_PARAM_AUTH", "W_FRESHNESS_CHANNEL", "W_WRITE_CONFLICT",
        ] {
            assert!(seen.insert(s), "duplicate code: {}", s);
        }
    }
}
