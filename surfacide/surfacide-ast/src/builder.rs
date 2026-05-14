//! Smart constructors for common AST shapes. These are not strictly
//! necessary — every AST type has public fields — but they reduce
//! boilerplate in tests and in `surfacide-syntax`'s CST→AST converter.

use crate::ident::Ident;
use crate::span::{FileId, Span};

/// Create a span in file 0 (test-only convenience).
#[doc(hidden)]
pub fn _test_span(start: u32, end: u32) -> Span {
    Span::new(FileId(0), start, end)
}

/// Create an identifier in file 0 (test-only convenience).
#[doc(hidden)]
pub fn _test_ident(name: &str) -> Ident {
    Ident::new(name, _test_span(0, name.len() as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slot::{SlotKind, AvailabilityValue};

    #[test]
    fn test_helpers_produce_well_formed_values() {
        let id = _test_ident("foo");
        assert_eq!(id.name, "foo");
    }

    #[test]
    fn availability_lattice_total() {
        // sanity: every pair is comparable
        for a in [AvailabilityValue::Critical, AvailabilityValue::BestEffort,
                  AvailabilityValue::MaintenanceWindow, AvailabilityValue::ReadOnlyFailover] {
            for b in [AvailabilityValue::Critical, AvailabilityValue::BestEffort,
                      AvailabilityValue::MaintenanceWindow, AvailabilityValue::ReadOnlyFailover] {
                let _ = a.rank().cmp(&b.rank());
            }
        }
    }

    #[test]
    fn slot_canonical_order_starts_with_idempotency() {
        assert_eq!(SlotKind::canonical_order()[0], SlotKind::Idempotency);
    }
}
