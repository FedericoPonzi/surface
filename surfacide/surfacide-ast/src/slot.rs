//! Slot values: idempotency, auth_channel, retention, rate_limit,
//! observability, availability, freshness.
//!
//! Each slot has a closed enum of legal values (§6.4.1). The grammar
//! parses any well-shaped value; this module is the typed representation
//! consumed by the slot pass.

use crate::expr::Expr as _Expr;  // imported so other modules can reference via re-export
#[allow(unused_imports)]
use _Expr as Expr;
use crate::ident::Ident;
use crate::span::Span;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// One of the seven mandatory action slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SlotKind {
    Idempotency,
    AuthChannel,
    Retention,
    RateLimit,
    Observability,
    Availability,
    Freshness,
}

impl SlotKind {
    pub fn name(self) -> &'static str {
        match self {
            SlotKind::Idempotency => "idempotency",
            SlotKind::AuthChannel => "auth_channel",
            SlotKind::Retention => "retention",
            SlotKind::RateLimit => "rate_limit",
            SlotKind::Observability => "observability",
            SlotKind::Availability => "availability",
            SlotKind::Freshness => "freshness",
        }
    }

    /// Canonical order in which slots appear in action bodies (and in
    /// docs projection). Out-of-order slots are `E_SURFACE_SLOT_ORDER`.
    pub fn canonical_order() -> [SlotKind; 7] {
        [
            SlotKind::Idempotency,
            SlotKind::AuthChannel,
            SlotKind::Retention,
            SlotKind::RateLimit,
            SlotKind::Observability,
            SlotKind::Availability,
            SlotKind::Freshness,
        ]
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "idempotency" => SlotKind::Idempotency,
            "auth_channel" => SlotKind::AuthChannel,
            "retention" => SlotKind::Retention,
            "rate_limit" => SlotKind::RateLimit,
            "observability" => SlotKind::Observability,
            "availability" => SlotKind::Availability,
            "freshness" => SlotKind::Freshness,
            _ => return None,
        })
    }
}

/// A parsed slot assignment, before validation. The value may be a
/// well-formed enum case, an unknown identifier (which the slot pass
/// rejects), or a `waived: "..."` form.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SlotAssign {
    pub kind: SlotKind,
    pub value: SlotValue,
    pub span: Span,
}

/// Where the effective slot value came from after [`crate::surface::DefaultsBlock`]
/// elaboration. Preserved through the AST so the docs projection can
/// render `(default)` / `(internal-preset)` tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SlotProvenance {
    Explicit,
    Default,
    InternalActionPreset,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SlotValue {
    Idempotency(IdempotencyValue),
    AuthChannel(AuthChannelValue),
    Retention(RetentionValue),
    RateLimit(RateLimitValue),
    Observability(ObservabilityValue),
    Availability(AvailabilityValue),
    Freshness(FreshnessValue),
    Waived { reason: String, reason_span: Span },
    /// A token we couldn't classify. Diagnostic emitted by the slot
    /// pass: `E_SURFACE_SLOT_UNKNOWN_VALUE`.
    Unknown(Ident),
}

// ---- per-slot closed enums ----

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IdempotencyValue {
    Idempotent,
    IdempotentBy(Vec<Ident>),
    AtMostOnce,
    AtLeastOnce,
}

/// A `auth_channel` value is a non-empty set of channels (v0.10).
/// A bare value `c` is sugar for `{c}`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AuthChannelValue {
    pub channels: Vec<AuthChannelTag>,
    pub set_form: bool, // distinguishes `{a, b}` from bare `a` for diagnostics
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AuthChannelTag {
    Session,
    BearerToken,
    SignedRequest,
    CapabilityUrl,
    Mtls,
    TrustedCaller,
    Anonymous,
}

impl AuthChannelTag {
    pub fn name(self) -> &'static str {
        match self {
            AuthChannelTag::Session => "session",
            AuthChannelTag::BearerToken => "bearer_token",
            AuthChannelTag::SignedRequest => "signed_request",
            AuthChannelTag::CapabilityUrl => "capability_url",
            AuthChannelTag::Mtls => "mtls",
            AuthChannelTag::TrustedCaller => "trusted_caller",
            AuthChannelTag::Anonymous => "anonymous",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "session" => AuthChannelTag::Session,
            "bearer_token" => AuthChannelTag::BearerToken,
            "signed_request" => AuthChannelTag::SignedRequest,
            "capability_url" => AuthChannelTag::CapabilityUrl,
            "mtls" => AuthChannelTag::Mtls,
            "trusted_caller" => AuthChannelTag::TrustedCaller,
            "anonymous" => AuthChannelTag::Anonymous,
            _ => return None,
        })
    }

    pub fn all() -> &'static [AuthChannelTag] {
        &[
            AuthChannelTag::Session,
            AuthChannelTag::BearerToken,
            AuthChannelTag::SignedRequest,
            AuthChannelTag::CapabilityUrl,
            AuthChannelTag::Mtls,
            AuthChannelTag::TrustedCaller,
            AuthChannelTag::Anonymous,
        ]
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RetentionValue {
    Ephemeral,
    Transactional,
    /// `audit(period=<DurationConst>)`
    Audit { period: Ident },
    /// `pii(class=<PiiClass>, ttl=<DurationConst>)`
    Pii { class: PiiClass, ttl: Ident },
    Secret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PiiClass {
    Name,
    Email,
    Phone,
    Address,
    Biometric,
    Financial,
    Health,
    Location,
    Identifier,
    Other,
}

impl PiiClass {
    pub fn name(self) -> &'static str {
        match self {
            PiiClass::Name => "name",
            PiiClass::Email => "email",
            PiiClass::Phone => "phone",
            PiiClass::Address => "address",
            PiiClass::Biometric => "biometric",
            PiiClass::Financial => "financial",
            PiiClass::Health => "health",
            PiiClass::Location => "location",
            PiiClass::Identifier => "identifier",
            PiiClass::Other => "other",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "name" => PiiClass::Name,
            "email" => PiiClass::Email,
            "phone" => PiiClass::Phone,
            "address" => PiiClass::Address,
            "biometric" => PiiClass::Biometric,
            "financial" => PiiClass::Financial,
            "health" => PiiClass::Health,
            "location" => PiiClass::Location,
            "identifier" => PiiClass::Identifier,
            "other" => PiiClass::Other,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RateLimitValue {
    /// `per_actor(<n>: Nat, per: <DurationConst>)`
    PerActor { n: u64, period: Ident },
    /// `per_target(<arg>, <n>, per: <DurationConst>)`
    PerTarget { arg: Ident, n: u64, period: Ident },
    /// `global(<n>, per: <DurationConst>)`
    Global { n: u64, period: Ident },
    Unlimited,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ObservabilityValue {
    CallerOnly(Vec<Ident>),
    Target { arg: Ident, events: Vec<Ident> },
    Broadcast(Vec<Ident>),
    Silent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AvailabilityValue {
    Critical,
    BestEffort,
    MaintenanceWindow,
    ReadOnlyFailover,
}

impl AvailabilityValue {
    pub fn name(self) -> &'static str {
        match self {
            AvailabilityValue::Critical => "critical",
            AvailabilityValue::BestEffort => "best_effort",
            AvailabilityValue::MaintenanceWindow => "maintenance_window",
            AvailabilityValue::ReadOnlyFailover => "read_only_failover",
        }
    }

    /// Lattice order for R-AVAIL-CONSISTENCY: stricter > weaker.
    /// critical > read_only_failover > maintenance_window > best_effort
    pub fn rank(self) -> u8 {
        match self {
            AvailabilityValue::BestEffort => 0,
            AvailabilityValue::MaintenanceWindow => 1,
            AvailabilityValue::ReadOnlyFailover => 2,
            AvailabilityValue::Critical => 3,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "critical" => AvailabilityValue::Critical,
            "best_effort" => AvailabilityValue::BestEffort,
            "maintenance_window" => AvailabilityValue::MaintenanceWindow,
            "read_only_failover" => AvailabilityValue::ReadOnlyFailover,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum FreshnessValue {
    Strong,
    /// `bounded(<epoch_name>, n=<k>)`
    Bounded { epoch: Ident, n: u64 },
    /// `eventual(<epoch_name>?)`
    Eventual { epoch: Option<Ident> },
    /// `stale_while_revalidate(<epoch_name>, n=<k>)`
    StaleWhileRevalidate { epoch: Ident, n: u64 },
    NotApplicable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_canonical_order_is_seven_distinct() {
        let order = SlotKind::canonical_order();
        let mut set = std::collections::HashSet::new();
        for s in order {
            assert!(set.insert(s));
        }
        assert_eq!(set.len(), 7);
    }

    #[test]
    fn slot_from_name_round_trip() {
        for s in SlotKind::canonical_order() {
            assert_eq!(SlotKind::from_name(s.name()), Some(s));
        }
    }

    #[test]
    fn auth_channel_round_trip() {
        for tag in AuthChannelTag::all() {
            assert_eq!(AuthChannelTag::from_name(tag.name()), Some(*tag));
        }
    }

    #[test]
    fn availability_rank_orders_stricter_higher() {
        assert!(AvailabilityValue::Critical.rank() > AvailabilityValue::BestEffort.rank());
        assert!(AvailabilityValue::ReadOnlyFailover.rank() > AvailabilityValue::MaintenanceWindow.rank());
    }

    // Suppress unused warnings on imports we'll wire later
    #[allow(dead_code)]
    fn _unused(_e: Expr, _s: Span) {}
}
