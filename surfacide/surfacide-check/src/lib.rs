//! Slot pass and cross-slot consistency checks.
//!
//! - [`slots`]: §6.4 mandatory-slot pass (presence, order, well-typed
//!   values, waiver-empty, duplicates), with defaults + internal_action
//!   elaboration per §6.4.6.1.
//! - [`derived`]: derived-field assignment static error (§6.6).
//! - Cross-slot consistency (auth_channel ↔ authentication;
//!   retention secret flow) is sketched as a stub and will be filled in
//!   once we have substrate side data flowing through resolve.

pub mod slots;
pub mod derived;
pub mod cross_slot;

use surfacide_ast::Project;
use surfacide_diag::Diagnostic;

#[derive(Debug, Default)]
pub struct CheckOutput {
    pub diagnostics: Vec<Diagnostic>,
}

pub fn run_slot_pass(project: &Project) -> CheckOutput {
    let mut diagnostics = slots::run(project);
    diagnostics.extend(derived::run(project));
    diagnostics.extend(cross_slot::run(project));
    CheckOutput { diagnostics }
}

pub fn run_cross_slot(project: &Project) -> CheckOutput {
    CheckOutput { diagnostics: cross_slot::run(project) }
}
