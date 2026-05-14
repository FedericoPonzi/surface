# Surface — open issues backlog

> **Snapshot after v0.10.1.** v0.10 closed the R7 unanimous backlog
> and the longest-standing structural friction (`derived` state).
> R8 (under v0.10) confirmed convergence and surfaced two P0s, both
> closed in the v0.10.1 spec patch:
> - `auth_channel` channel-agnostic `[*]` realises form
> - `internal_action` vs `defaults` precedence pinned
>
> With those two patches the **P0 queue is empty**. Remaining open
> items are P1 design work (label calculus, discharge separation)
> already targeted at v0.11, plus a steady-state P2/P3 backlog.

- **Priority:** P0 (blocking; users hit it in normal use), P1 (real
  pain but with workarounds), P2 (nice-to-have), P3 (deferred /
  explicitly cut from current scope).
- **Difficulty:** S (hours), M (days), L (a week+), XL (research-grade
  or design work needed first).
- **Source:** the round and agent that flagged it. Multi-agent flags
  marked `(unanimous)`.

---

## P0 — (none open)

The two R8 P0s closed in v0.10.1. See "Closed in v0.10.1" below.

## P1 — real pain in any non-trivial spec (open)

| ID  | Title                                                                  | Difficulty | Source         | Notes |
|-----|------------------------------------------------------------------------|:----------:|----------------|-------|
| P1‑10 | Property bodies remain a non-technical-reader cliff                    | M | R2 / R3 / R5 / R7 (every round) | Mitigated by v0.9 slot checklist + v0.10 docs projection. Spec text remains a wall. |
| P1-V10-1 | Scenario `given` with `derived` fields is awkward | S | R8 Opus | A `given edge_has == { … }` is illegal; author must set substrate state and rely on the projection. Compiler error message must point at the underlying substrate fields. PL review §147 open question. |
| P1-V10-2 | `internal_action` as keyword vs modifier | S | R8 GPT‑5.5 + PL review | A modifier (`action … mode: internal`) would compose with substrate-level actions; v0.10's keyword form is surface-only. Opus's `InvalidationQueue.rollback` is "internal in spirit" but can't carry it. v0.11 candidate. |
| P1-V10-3 | Formal label calculus for retention / information flow | L | R7 / R8 / PL review | v0.10 commits to explicit-flow-only. PL review proposed a typed label lattice + join/meet rules; AST `expr_dep` extraction needs to be spelled out for `choose`, `aggregate`, `state_at(e)`, observables. Parked to v0.11. |
| P1-V10-4 | Discharge vs acknowledgement separation in §15.3 | M | R7 / R8 / PL review | `acknowledged because:` is "structured comments" for high-severity rules. v0.11 should distinguish (typechecker-recognised) discharge from (audited) acknowledgement; emitted docs / proof artifacts should show acceptance of risk separately from satisfaction. |
| P1-V10-5 | Cross-substrate `internal_action` (substrate-level form) | S | R8 Opus | Some substrate actions (e.g. operator rollback writing a `cross_visible` aux) are internal in spirit but plain syntax. Either add `substrate component internal action …` or accept the gap. |

## P2 — nice-to-have, smaller wins (open)

| ID  | Title                                                                  | Difficulty | Source         | Notes |
|-----|------------------------------------------------------------------------|:----------:|----------------|-------|
| P2‑1 | Static rejection of overlapping `raises` guards may be undecidable     | S | R5 reviewer | Beyond simple finite domains the check is undecidable. Soften to a warning + best-effort static check. |
| P2‑2 | `fairness weak <surface_action>` shorthand ambiguous in `compose`      | S | R4 Opus | When two partial substrates each realize a surface action, force the user to name the substrate action. |
| P2‑3 | Branch labels are action-local but no other namespacing rules stated   | S | R5 reviewer | Document a name-resolution order. |
| P2‑4 | Map-key story for `cross` partially specified                          | S | R3 Sonnet     | No canonical pattern for `Map[(K1, K2) -> V]` access in `aggregate` over a `cross` scope. |
| P2‑5 | Channel name vs. component name namespacing                            | S | R4 Opus       | State explicitly that they're disjoint. |
| P2‑6 | A non-trivial `attacker` example exercising signed URLs                | S | R3 / R4       | Signed-URL attackers use a different shape than geo-bypass. |
| P2‑7 | `system` actor distinguished from a `User` actor                       | S | R3 Opus       | No example shows when `surface_actor … = system` is right. |
| P2-V10-1 | `replicate` IDs as indices into typed maps                            | S | R7 / R8 Opus | `Map[InvId -> Set[String]]` instead of `Map[InvId -> Set[EdgeId]]`. Carried since v0.9. |
| P2-V10-2 | Slots-as-effect-row refactor                                          | M | PL review | Effect-system framing of slots; `defaults` would become row-polymorphic elaboration. No behaviour change if done. Parked to v0.11. |
| P2-V10-3 | `surface check --no-substrate` semantics for `derived` fields         | S | PL review §147 / R8 Opus | What does scenario / property checking do for derived fields when no substrate is selected? Currently undefined. |

## P3 — explicitly deferred / parked

| ID  | Title                                                                  | Difficulty | Source         | Notes |
|-----|------------------------------------------------------------------------|:----------:|----------------|-------|
| P3‑1 | Assume/guarantee fairness for cross-module liveness                    | XL | R3 FM critique | v0.11 target (slipped from v0.8, v0.10). |
| P3‑2 | Confidentiality / 2-safety attackers                                   | XL | R3 FM critique | Hyperproperty encoding. |
| P3‑3 | `surface diff` between two spec versions                               | XL | R3 lang critique | Research-grade. |
| P3‑4 | Module value parameters at the module level                            | M  | R3 lang critique | Use `replicate` / `extern`. |
| P3‑5 | Channel semantics enums (`unordered`/`at_least_once`/`exactly_once`)   | M  | R3 lang critique | `exactly_once` doesn't really exist. |
| P3‑6 | Inter-instance messaging within one `replicate`                        | M  | R4 Opus       | Use a third component. |
| P3‑7 | SDK code generation                                                    | L  | R3 lang critique | Tests are written by hand for now. |
| P3‑8 | Real wall-clock time                                                   | L  | R3 FM critique, R7 unanimous | v0.10 ships *symbolic* epochs (§7.2.4); wall-clock parked. |
| P3‑9 | `attacker B extends A` inheritance                                     | S  | R3 lang critique | Compose by copy/paste. |
| P3‑10 | Author-extensible obligation rules                                    | L  | v0.9 design   | Closed by design; revisit only if catalog stabilises. |

---

## Categorised view (open, post-v0.10)

### "Spec is genuinely incomplete" (needs design work)

P1-V10-3 (formal label calculus for retention/info-flow) and P1-V10-4
(discharge vs acknowledgement separation) are the two remaining
structural gaps, both explicitly parked to v0.11 with rationale.
The longest-standing structural friction (`derived` state, raised
R5/R6/R7) was closed in v0.10.

### "v0.10 ergonomics that need follow-up"

P0-V10-1 (`auth_channel` set desugaring is Cartesian) is the only
R8 unanimous blocker and the v0.10.1 target. P0-V10-2 (defaults vs
internal_action precedence) is a one-line spec patch.

### "Property bodies still aren't friendly enough"

P1‑10. Partially mitigated by the v0.9 slot-checklist projection and
the v0.10 derived-state simplification. Spec-text wall for nested
quantifiers remains.

### "Polish wins waiting for compiler reality"

The P2 items. None block correct use; revisit when the compiler
exists and we have real‑world feedback.
