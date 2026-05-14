//! Surface language AST types.
//!
//! This crate is the data model for the entire frontend. It depends on
//! nothing internal: every other crate sits "above" it.
//!
//! Key invariants:
//!
//! - Every node carries a [`Span`] pointing at its source. Spans must
//!   survive every transformation; later passes need them for diagnostics.
//! - Identifiers are interned via [`Ident`]; the source spelling is
//!   preserved.
//! - The AST mirrors the v0.10.1 spec structure: a [`Module`] contains
//!   declarations, a [`SurfaceBlock`] contains state + actions + defaults
//!   + internal_actions + observables + properties, etc.
//!
//! Construction helpers live in [`builder`]; tests cover the smart
//! constructors. Higher-level passes consume the AST read-only and emit
//! diagnostics via the `surfacide-diag` crate.

pub mod span;
pub mod ident;
pub mod ty;
pub mod expr;
pub mod decl;
pub mod surface;
pub mod substrate;
pub mod slot;
pub mod compose;
pub mod scenario;
pub mod project;
pub mod builder;

pub use span::{Span, FileId, FileRegistry};
pub use ident::{Ident, QualifiedName};
pub use ty::{Type, TypeKind, RecordTypeField};
pub use expr::{Expr, ExprKind, Binding};
pub use decl::{Decl, ModuleDecl, ModuleFile, ModuleHeader, UseDecl, ActorDecl, EventDecl,
               EventField, TypeAliasDecl, ConstDecl, ExternDecl, ObservableDecl, ActorBinder,
               Param, HistoryPredicateDecl, AttackerDecl, AttackerCapability};
pub use surface::{SurfaceBlock, StateField, StateFieldKind, RetentionClass, ActionDecl,
                  InternalActionDecl, DefaultsBlock, RaisesClause, EffectStmt, EffectBlock,
                  BranchLabel, Property, PropertyKind};
pub use substrate::{SubstrateBlock, Component, ReplicateBlock, ChannelDecl, ChannelMultiplicity,
                    ReceivesHandler, SendsStmt, MapsBlock, RealizesBlock, RealizesClause,
                    RealizesTarget, InternalBlock, AuthenticationBlock, AuxiliaryBlock,
                    AuxiliaryVar, AuxiliaryKind, EpochDecl, FairnessSpec};
pub use slot::{SlotKind, SlotValue, SlotAssign, SlotProvenance,
               IdempotencyValue, AuthChannelValue, RetentionValue, RateLimitValue,
               ObservabilityValue, AvailabilityValue, FreshnessValue, PiiClass};
pub use compose::{ComposeBlock, AcknowledgedBlock, AcknowledgedEntry, ObligationKind,
                  WriteConflictResolution};
pub use scenario::{Scenario, ScenarioKind, ScenarioClause};
pub use project::Project;
