---
title: Coverage Manifest
description: Every dimension Surface covers, which construct addresses it, since when, and what Surface deliberately does not cover.
show_table_of_contents: true
---
# Surface — Coverage Manifest (v0.10.1)

> Companion to [`language-spec.md`](language-spec.md). Read
> [`overview.md`](overview.md) first for the pitch.
>
> **Purpose.** Surface promises to make the *boundaries* of a system
> visible. This file enumerates **every dimension** Surface covers,
> **which construct** addresses each, **since when**, and crucially
> **what Surface deliberately does not cover** in this version.
>
> If a dimension is not in §1 and not in §3, it is **out of scope for
> v0.10.1** — neither covered nor deliberately excluded; it just hasn't
> been considered yet. Such dimensions are tracked in
> [`../TODO.md`](../TODO.md) and are a normal feedback channel from
> R-round agents.
>
> This document is normative. A Surface conformance test must:
> - Reject every spec missing a §1 *required* construct.
> - Not silently address any §3 *excluded* dimension.

---

## 1. Dimensions Surface covers

The table is read as: **dimension → the language construct that
addresses it → since-version → status**.

`Status` codes:

- **R** = required (compiler errors if absent).
- **S** = supported (available; opt-in or context-dependent).
- **D** = derived (computed/checked from other declarations; no explicit
  user-facing construct).

### 1.1 Structural

| Dimension                              | Construct                                        | Since | Status |
|----------------------------------------|--------------------------------------------------|:-----:|:------:|
| Module identity & composition          | `module <QualifiedName>`                         | v0.2  | R |
| Multi-file modules                     | files unioned by `module` header                  | v0.4  | S |
| Nested / sub-modules                   | qualified module names; `module Foo.Bar`         | v0.2  | S |
| Module visibility                      | `module … private`                                | v0.2  | S |
| External configuration                 | `extern <Name> : <Type>`                          | v0.3  | S |
| Model parameters                       | `const <Name> : <Type>`                           | v0.2  | S |

### 1.2 Domain modelling

| Dimension                              | Construct                                        | Since | Status |
|----------------------------------------|--------------------------------------------------|:-----:|:------:|
| Types (Nat/Int/Bool/Set/Seq/Map/…)     | §3 type grammar                                  | v0.2  | R |
| Records & tagged unions                | `{ field: T, … }`; `A | B | C`                   | v0.2  | S |
| Map totality                           | `Map[K -> V]` partial; total via comprehension   | v0.7  | R |
| Cartesian products & tuple keys        | `A cross B`; `(a, b)`                            | v0.6  | S |
| Actors & subtyping                     | `actor A`; `actor B extends A`                   | v0.3  | R |
| Events (typed observations)            | `event Name(…)`                                   | v0.3  | R |
| Observables (pure derived views)       | `observable name(…) : T = expr`                   | v0.4  | S |
| Actor-relative observables             | `observable for u: <Actor> …`                     | v0.9  | S |
| `derived` surface state               | `state { f: T derived [shape: …] }` (projection lives in substrate's `maps`) | v0.10 | S |
| State-field retention class            | `state { f: T retention: <cls> }`                  | v0.10 | S |
| `private` state-field modifier         | `state { f: T private }`                           | v0.10 | S (convention) |

### 1.3 Behaviour

| Dimension                              | Construct                                        | Since | Status |
|----------------------------------------|--------------------------------------------------|:-----:|:------:|
| Surface state & actions                | `surface { state {…} action …(…) … }`             | v0.2  | R |
| Positive preconditions                 | `when <Pre>`                                      | v0.2  | R |
| Error model                            | `raises { Name when G }`                          | v0.3  | R |
| Branch coverage                        | labeled `if/else`; project-level coverage check  | v0.7  | R |
| Bounded iteration                      | `for x in S do …`                                 | v0.4  | S |
| Optional unwrap                        | `if let Some(x) := … then …`                      | v0.7  | S |
| Choose / Hilbert epsilon               | `choose <name>: T. P`; `choose x in S. P`         | v0.4  | S |
| Aggregation across replicas            | `aggregate Comp[id].expr using <agg>`              | v0.6  | S |
| Atomic step (within `then`)            | one TLA+ next-state disjunct per leaf branch     | v0.2  | D |
| Action return values                   | `action … -> T … return e`                        | v0.3  | S |

### 1.4 Mandatory coverage slots on actions (v0.9, expanded v0.10)

Each slot is a closed enum with explicit `waived: "<reason>"` escape.
v0.10 adds `freshness` (seventh slot) and introduces `defaults { … }`
and `internal_action` for ceremony reduction (§6.4.5 / §6.4.6).

| Slot            | Question                                                              | Since |
|-----------------|------------------------------------------------------------------------|:-----:|
| `idempotency`   | Is replay safe? Keyed by what?                                        | v0.9 |
| `auth_channel`  | How does the caller's identity arrive (now a SET, v0.10)?             | v0.9 / v0.10 set form |
| `retention`     | What data class is touched, and how long is it retained?              | v0.9 |
| `rate_limit`    | What rate is permitted per actor/target/global?                       | v0.9 |
| `observability` | Which actors learn that this action happened, via which events?       | v0.9 |
| `availability`  | What is this action's expected availability class?                    | v0.9 |
| `freshness`     | How stale may the result be (symbolic epochs)?                        | v0.10 |

See `language-spec.md` §6.4 for the closed enum of legal values per
slot. The slot pass (`surface check --slots`) is a precondition for
codegen.

**v0.10 ceremony-reduction constructs**:

- `defaults { … }` (§6.4.5): a top-level block inside `surface` that
  fills slots not explicitly assigned per action. Explicit per-action
  overrides take precedence. Every slot still resolves to a definite
  value; no "implicit waiver" via omission.
- `internal_action` (§6.4.6): a keyword that auto-fills four slots
  (`auth_channel: trusted_caller`, `rate_limit: waived: "internal"`,
  `availability: maintenance_window`, and an implicit `internal`
  marker for the docs projection). Three slots remain mandatory
  (`idempotency`, `retention`, `observability`, `freshness`).

### 1.5 Refinement (surface ↔ substrate)

| Dimension                              | Construct                                        | Since | Status |
|----------------------------------------|--------------------------------------------------|:-----:|:------:|
| State projection                       | `maps { surface_field = expr }`                   | v0.2  | R |
| Action realisation                     | `realizes { surface.A(…) by Comp.a when … }`      | v0.2  | R |
| Internal (non-realising) actions       | `internal { Comp.action; Comp.*; … }`              | v0.3  | R |
| External / out-of-band actions         | `realizes { surface.A(…) by EXTERNAL }`            | v0.4  | S |
| Stutter for no-effect branches         | `realizes { surface.A[label] by stutter }` (compose)| v0.7  | S |
| Default-deny on substrate actions      | every substrate action ∈ `realizes ∪ internal`    | v0.3  | R |
| Auxiliary variables                    | `auxiliary { history h := … ; prophecy p := * }`   | v0.3  | S |
| Partial field ownership                | `partial substrate … owns { … }`                  | v0.5  | S |
| Compose of peer substrates             | `compose <N> = <S1> + <S2> { … }`                  | v0.5  | S |
| Cross-substrate guards                 | `realizes { … when <other_substrate>.<aux> … }`    | v0.6  | S |
| Cross-substrate aux (two-way contract) | `aux … cross_visible invariant …`                  | v0.7  | S |
| Replicated components                  | `replicate C[id in IDS] { … }`                     | v0.4  | S |
| FIFO channels (with multiplicity)      | `channel C { from A[…] to B[…] }`                  | v0.4  | S |
| Cross-substrate channels               | `channel C { … }` inside `compose`                 | v0.5  | S |
| Fairness                               | `fairness weak/strong A`                          | v0.3  | R for liveness |
| Error refinement                       | mirror-guard or disable-action pattern (§6.2.1)   | v0.7  | R |

### 1.6 Verification artefacts

| Dimension                              | Construct                                        | Since | Status |
|----------------------------------------|--------------------------------------------------|:-----:|:------:|
| Safety properties                      | `property name { always P }`                      | v0.3  | S |
| Liveness properties                    | `property name { eventually P }`                  | v0.3  | S |
| Scenarios (executable use cases)       | `scenario "…" kind: … { … }`                       | v0.3  | S |
| Forbidden traces                       | `scenario … kind: forbidden { … }`                 | v0.3  | S |
| Reusable history predicates            | `history_predicate name(…) { … }`                  | v0.5  | S |
| Time-relative state snapshots          | `state_at(e)`                                     | v0.7  | S |
| Event-log helpers                      | `events_before` / `events_after` / `first` / `last` / `count` / `between` | v0.6 | S |

### 1.7 Security

| Dimension                              | Construct                                        | Since | Status |
|----------------------------------------|--------------------------------------------------|:-----:|:------:|
| Integrity reachability                 | `attacker A { controls … goal … }`                | v0.3  | S |
| Authentication mapping                 | `authentication { surface_actor of … = … }`        | v0.3  | R when attacker is run |
| Permission predicates                  | Boolean `observable` referenced from `when`        | v0.7  | S |

### 1.8 Static obligations (v0.10 — full catalog, §15)

| Dimension                              | Obligation kind                                   | Rule              | Severity | Since |
|----------------------------------------|--------------------------------------------------|-------------------|:--------:|:-----:|
| Availability dependence (read)         | `availability_depends_on(C)`                      | R-AVAIL-READ      | medium  | v0.9 |
| Availability dependence (channel)      | `availability_depends_on(C)`                      | R-AVAIL-CHANNEL   | medium  | v0.9 |
| Availability declared vs closure       | `availability_consistency(...)`                   | R-AVAIL-CONSISTENCY | high  | v0.10 |
| Channel reliability class mismatch     | `availability_channel_class(...)`                 | R-AVAIL-CHANNEL-CLASS | high | v0.10 |
| Trust transitivity                     | `trust_transitive(C)`                             | R-TRUST-PARAM-AUTH | high   | v0.9 |
| Concurrent write conflicts             | `write_conflict(field)`                           | R-WRITE-CONFLICT  | high    | v0.9 |
| Replay amplification                   | `replay_amplification(action)`                    | R-REPLAY-AMP      | medium  | v0.9 |
| Retention propagation across actions   | `retention_propagation(src, dst)`                 | R-RETENTION-FLOW  | high    | v0.9 |
| Information flow (state → sink)        | `information_flow(src, sink)`                     | R-INFO-FLOW       | high    | v0.10 |
| Anonymous action reads PII             | `pii_anon(action, field)`                         | R-PII-ANON        | medium  | v0.10 |
| Actor-view leak                         | `actor_view_leak(obs, actor_var)`                 | R-ACTOR-VIEW-LEAK | high (hard error) | v0.9 |
| Derived-state assignment                | `derived_write(field, action)`                    | R-DERIVED-WRITE   | high (hard error) | v0.10 |
| Strong freshness without sync ack      | `freshness_channel(action, channel)`              | R-FRESHNESS-CHANNEL | high  | v0.10 |

**Framework remains closed** — new rules enter only via versioned
spec changes (v0.10 added six; v1.0 will freeze the framework
itself).

### 1.9 Output artefacts

| Dimension                              | Construct                                        | Since | Status |
|----------------------------------------|--------------------------------------------------|:-----:|:------:|
| TLA+ codegen (surface)                 | `surface emit tla`                                | v0.3  | S |
| PlusCal codegen (substrate)            | `surface emit pluscal`                            | v0.3  | S |
| Gherkin skeletons                      | `surface emit gherkin`                            | v0.3  | S |
| Markdown docs (with slot checklist)    | `surface emit docs`                               | v0.3 (slots v0.9) | S |

---

## 2. Mandatory action slots — detailed reference

Reproduced from `language-spec.md` §6.4 for review-without-cross-referencing.
The normative form is in the spec.

| Slot            | Closed enum (abridged)                                                                                                |
|-----------------|------------------------------------------------------------------------------------------------------------------------|
| `idempotency`   | `idempotent` \| `idempotent by(<args>)` \| `at_most_once` \| `at_least_once` \| `waived`                                |
| `auth_channel`  | `session` \| `bearer_token` \| `signed_request` \| `capability_url` \| `mtls` \| `trusted_caller` \| `anonymous` \| `waived` |
| `retention`     | `ephemeral` \| `transactional` \| `audit(period=D)` \| `pii(class=C, ttl=D)` \| `secret` \| `waived`                     |
| `rate_limit`    | `per_actor(n, per=D)` \| `per_target(arg, n, per=D)` \| `global(n, per=D)` \| `unlimited` \| `waived`                    |
| `observability` | `caller_only(E, …)` \| `target(arg, E, …)` \| `broadcast(E, …)` \| `silent` \| `waived`                                  |
| `availability`  | `critical` \| `best_effort` \| `maintenance_window` \| `read_only_failover` \| `waived`                                  |

**Waivers are not silence.** A `waived: "<reason>"` slot is *visible*
in source, in the docs projection, and in the obligation pass output.
A reviewer can grep for `waived:` and audit every one. The intent is
that "this dimension does not apply to this action" is a *positive*
design decision, not the absence of a decision.

---

## 3. Deliberately excluded from v0.9

Each entry: **what is excluded, why, where you would expect to find it
if Surface had it, and the targeted version if any.**

### 3.1 Confidentiality / non-interference / 2-safety

**Excluded because:** confidentiality is a hyperproperty (a relation
between *pairs* of executions, not a property of a single execution).
TLA+ reachability cannot express it directly; encoding requires
Apalache-only tricks. The R3 formal-methods critique deemed this out
of scope. v0.9 maintains the v0.3 decision.

**Workaround:** model the *integrity-side* of confidentiality
("attacker causes effect Y") with `attacker` blocks (§10). Pure
read-only leaks remain out of scope.

**Target version:** none. Will be revisited only if a concrete use
case demands it.

### 3.2 Real (wall-clock) time

**Excluded because:** introducing real-valued time complicates the
semantics for rare cases. **v0.10 introduces *symbolic* time** via
`freshness: bounded(epochs=n)` and `stale_while_revalidate` (§1.4 + spec
§6.4.1). An "epoch" is a count of substrate propagation steps; an
"epoch" is not a second. The `retention: audit(period=D)` slot uses a
`Duration` const for documentation but the value is not used in
checking. Wall-clock time remains parked.

**Target version:** none planned. Real-time attackers and duration
arithmetic would require a different checker (TLA+ Timed, etc.) and
are not currently a need.

### 3.3 Cross-module liveness composition

**Excluded because:** liveness does not compose by trace inclusion;
proving an `eventually` at the parent level from `eventually`s at the
child level requires assume/guarantee fairness, which is
research-grade.

**Target version:** v0.11 (was provisionally targeted at v0.8 in the
v0.7 docs, then v0.10; v0.10 spent its budget on slot hardening and
the rule catalog).

### 3.4 Author-extensible obligation rules

**Excluded because:** allowing organisations to add rules would let
two Surface specs mean different things in different orgs. The
closed catalog (§15) is the language; new rules enter only via
versioned spec changes. v0.10 expanded the catalog from 6 to 12
built-in rules; v1.0 will freeze the framework but keep the catalog
language-versioned.

**Target version:** not currently planned.

### 3.5 `surface diff` between spec versions

**Excluded because:** bidirectional refinement on potentially renamed
actions/state is research-grade. Dropped from v0.3, still dropped.

**Target version:** none.

### 3.6 Inter-instance messaging inside one `replicate`

**Excluded because:** `Edge[i].sends to Edge[j]` would require a more
elaborate channel typing rule. Use a third component (a bus) or a
shared channel via `compose`.

**Target version:** none planned.

### 3.7 Attacker inheritance

**Excluded because:** compose by copy/paste is fine while there is no
real-world `attacker` library. Revisit when an actual reusable
attacker pattern emerges.

**Target version:** none planned.

### 3.8 Channel semantics enums

**Excluded because:** `exactly_once` doesn't really exist; modelling
unreliability with explicit `internal` actions on a plain FIFO
channel is the canonical approach and is more truthful.

**Target version:** none.

### 3.9 Module value parameters

**Excluded because:** `replicate` covers per-instance values inside a
substrate; `extern` covers module-level configuration. Adding a third
mechanism would be redundant.

**Target version:** none planned.

### 3.10 Confidence levels / probability

**Excluded because:** Surface is a discrete-state formal language.
Probabilistic refinement (Markov chain / Markov decision process)
would be a different language; can be revisited but is not on the
roadmap.

**Target version:** none planned.

### 3.11 Hierarchical compose

**Excluded because:** `compose` v0.5 joins partial substrates that
target the same surface. *Hierarchical* composition (parent surface
refined jointly by sub-module surfaces) is the v0.4 module/sub-module
story and is separate. Combining the two is not currently a real
need.

**Target version:** none planned.

---

## 4. Tooling matrix

| Tool invocation                       | Pass            | Where         | Catches                               |
|---------------------------------------|-----------------|---------------|---------------------------------------|
| `surface check --slots`               | Slot pass       | Frontend      | Missing/illegal §6.4 slots             |
| `surface check --obligations`         | Obligation pass | Frontend      | Unacknowledged §15 derived obligations |
| `surface check --refinement`          | Refinement      | TLA+ backend  | Substrate diverges from surface       |
| `surface check --liveness`            | Temporal        | TLA+ backend  | `eventually` violated under fairness  |
| `surface check --attacker <name>`     | Reachability    | TLA+ backend  | Integrity counter-example             |
| `surface check`                        | All of the above | Both         | Pipeline default                      |
| `surface emit tla` / `pluscal`        | Codegen         | Backend       | Produces `.tla` / PlusCal             |
| `surface emit docs`                   | Doc projection  | Frontend      | Markdown with slot checklist           |
| `surface emit gherkin`                | Test skeletons  | Frontend      | Gherkin from scenarios                 |
| `surface migrate v0.7 v0.9`           | Migration       | Frontend      | Inserts waivers; flags review sites    |

**Frontend vs. backend.** The frontend passes are deterministic and
local; they require no model checker and produce Rust-style errors
that point to a fix site. The backend passes use TLA+/PlusCal and
TLC/Apalache and produce counter-example traces. Most of Surface's
value (the slot pass + the obligation pass) is in the frontend.

> **v0.9 positioning.** TLA+/PlusCal is Surface's *primary backend*,
> not its semantics. The language has its own toolchain and would
> still be useful if a different backend (e.g. Lean, Coq, an Alloy
> embedding, or a bespoke checker) replaced TLA+. Surface specs are
> not "TLA+ with sugar"; they are boundary descriptions with a
> TLA+ codegen.

---

## 5. Compatibility / migration

### 5.1 v0.7 → v0.9

Three breaking changes:

1. **Mandatory action slots** (§6.4). Every action must carry the six
   slots or be migrated. The `surface migrate v0.7 v0.9` tool inserts
   `waived: "v0.7 migration; review"` at every missing slot; specs
   compile after migration but each waiver is a review site.
2. **`observable for <Actor>`** (§5.3) is a *new* construct.
   Pre-v0.9 specs continue to type-check; opting in is voluntary, but
   v0.9 examples migrate global actor-relative state to the new form.
3. **Obligation pass** (§15). Specs that import `cross_visible` aux
   variables, declare `param.` authentication, or fan messages out
   through channels will produce obligation warnings/errors on first
   `surface check`. Add an `acknowledged { … }` block or restructure.

The TLA+/PlusCal codegen is **unchanged** between v0.7 and v0.9. v0.9
is a *frontend* upgrade, not a semantics change.

### 5.2 v0.9 → v0.10

Six changes, three of which require source edits:

1. **`freshness:` slot is mandatory on every surface action.** The
   migrator inserts `freshness: waived: "v0.9 migration; review"`;
   replace per action.
2. **Slots apply to surface actions only.** Any v0.9 spec that
   accidentally put slots on substrate component actions is rejected;
   strip them.
3. **`auth_channel` may be a set.** Single-value usage is unchanged
   (one-element set is sugar); multi-channel actions migrate to set
   form.
4. **`acknowledged { … }` syntax tightened** (§15.3): `because:` is
   per-entry; duplicate cross-substrate acks must agree. v0.9 specs
   that used per-block `because:` need a one-line rewrite.
5. **`E_ACK_ORPHAN` is now `W_ACK_NO_RULE`** (warning, not error).
   Proactive acknowledgements are legal.
6. **New constructs** (opt-in, no breakage): `derived from` state
   fields (§6.6), `defaults { … }` block (§6.4.5), `internal_action`
   (§6.4.6), state-field `retention:` annotations (§6.5).

`surface migrate v0.9 v0.10` handles 1–5 mechanically; 6 is voluntary
adoption. The v0.10 examples (`url-shortener`, `twitter`) demonstrate
the new constructs.

The obligation rule catalog grew from 6 to ~12 rules; existing v0.9
specs may produce new warnings/errors from the v0.10 rules (notably
R-AVAIL-CONSISTENCY catches declared-vs-closure availability
mismatches that v0.9 did not flag).

### 5.3 The "70 → 100" framing (revisited at v0.10)

Before v0.9, Surface answered: "given that you have specified the
boundaries, does your implementation respect them?" — refinement
checking. v0.9 also answered: "have you specified the boundaries?" —
the slot pass and the obligation pass.

v0.10 fills in three things v0.9 promised but didn't deliver: the
**rule catalog is substantive** (12 rules, not 6); the **`derived`
state pattern is named** (closing a 3-round backlog item); and
**ceremony reduction** via `defaults` + `internal_action` brings
slot-heavy specs back into readable territory. Slots are still
mandatory, but actions inherit shared values and internal actions
get a sane preset.

Authors who maintain only v0.7-style refinement and waive every new
slot are still valid users — but their spec now shows, in source,
where the open boundaries are. That visibility is the v0.9/v0.10
deliverable.
