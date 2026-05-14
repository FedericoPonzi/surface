---
title: Changelog
description: Per-version Surface history from v0.1 through v0.10.1.
show_table_of_contents: true
---
# Surface — Changelog

Per-version history, folded back from each agent-evaluation round.

## v0.10.1 (current)

Spec patch closing the two R8 P0 findings. No new features; the
v0.10 design surface is unchanged.

- **§6.4.1 — channel-agnostic `[*]` realises form** (closes
  P0-V10-1, R8 unanimous). Substrates may write
  `surface.<A>(...)[*][<label>]` to cover every channel branch of a
  multi-value `auth_channel` action with one clause when the
  substrate does not differentiate between channels. Explicit
  per-channel clauses coexist and take precedence. The §6.1.2
  branch coverage rule is updated to recognise `[*]` as covering
  every channel branch.

  Resolves the R8 finding that Opus's CloudFront grew +70 LoC
  purely from `get`'s 3 channels × 2 body labels = 6 near-identical
  realises. With `[*]`, the same coverage is two clauses
  (`surface.get(d, p)[*][hit]` and `[*][miss]`).

- **§6.4.6.1 — slot resolution precedence** (closes P0-V10-2, R8
  Opus). The order is now normative:
  `internal_action preset < defaults < per-action slot line`.
  Reading: presets are a fallback, defaults express project intent,
  explicit lines win. The docs projection renders the effective
  value plus a provenance tag.

## v0.10

A double-duty round: address the R7 unanimous P0/P1 backlog *and*
close the longest-standing structural friction (`derived` surface
state, raised in R5, R6, R7).

**Highlights**:

- **`derived from <expr>` surface state (§6.6).** A surface state
  field can be declared as a projection of substrate state. Surface
  bodies cannot assign to it; the substrate's `maps` block satisfies
  the projection. Eliminates the "mirror substrate state in the
  surface and reassert it on every action" anti-pattern that has
  appeared in every CloudFront round since v0.5. Closes
  R5-audit/R6/R7 carry-forward.
- **`auth_channel` is a set** (§6.4.1). `auth_channel: { anonymous,
  signed_request, bearer_token }` covers multi-channel endpoints in
  one slot; bare value is sugar for a one-element set. Closes R7
  unanimous P0-V9-1.
- **`freshness` slot (§6.4.1).** Seventh mandatory slot. Symbolic
  epochs, not real clocks. Closed enum:
  `strong | bounded(epochs=n) | eventual | stale_while_revalidate(epochs=n) | not_applicable | waived`.
  Pairs with new obligation rule R-FRESHNESS-CHANNEL.
- **`defaults { … }` block (§6.4.5)** and **`internal_action`
  modifier (§6.4.6).** Two ceremony-reduction constructs that
  keep slot mandatoriness but cut ~70% of the per-action line count
  on slot-heavy specs. Closes R7 unanimous P1-V9-1/8.
- **State-field retention annotations (§6.5).** State fields may
  carry `retention: <cls>`, including a `private` modifier. Makes
  the §6.4.1 secret-flow check realisable. Closes R7 P0-V9-5.
- **Obligation rule catalog expanded from 6 to ~12 rules** (§15.4).
  New rules: R-AVAIL-CONSISTENCY, R-AVAIL-CHANNEL-CLASS, R-INFO-FLOW,
  R-PII-ANON, R-DERIVED-WRITE, R-FRESHNESS-CHANNEL. Each carries an
  explicit severity. Closes R7 unanimous P1-V9-2.

**Spec patches** (R7 P0/P1):

- §6.4 — slots apply to surface actions only; error codes renamed to
  `E_SURFACE_SLOT_*` (R7 P0-V9-3).
- §15.3 — `acknowledged { … }` syntax pinned: per-entry `because:`,
  duplicate-acknowledge agreement rule, single-action obligation form
  with worked example (R7 P0-V9-2).
- §15.4 — R-AVAIL-CHANNEL clarified: derives a substrate dependency,
  not a surface-slot value. Separated from R-AVAIL-CONSISTENCY which
  checks the closure vs. the declared slot (R7 P0-V9-4).
- §5.3 — rule 3 worked example for globally-keyed non-actor state in
  `observable for u: <Actor>` bodies (R7 P1-V9-5); auto-binding
  extends to substrate-side `realizes when …` (R7 P1-V9-6).
- §15.3 — `E_ACK_ORPHAN` downgraded to `W_ACK_NO_RULE` (warning, not
  error). Authors may proactively acknowledge intuitions the catalog
  hasn't codified (R7 P1-V9-9).

**Examples updated**:

- `examples/twitter/`: `posts` migrated to `derived from aggregate
  UserAccount[u].my_posts using union_set`. Uses `defaults { … }` to
  trim slot ceremony on the 5 actions.
- `examples/url-shortener/`: `defaults { … }` block; multi-value
  `auth_channel` on `visit`; `freshness` slots throughout.

**R8 evaluation pending** — GPT-5.5 and Opus 4.7 will each author a
v0.10 CloudFront spec after a staff-level PL designer review.

**PL designer review fold (pre-R8)**: A sync staff-level PL review
(see `docs/reviews/PL_REVIEW_V010.md`) was run between the v0.10 draft and R8.
Three of its "must address" findings were folded in before R8:

1. **`derived from <substrate-expr>` was the PL review's
   Cut-One pick** (least defensible v0.10 decision: violates the
   surface/substrate abstraction boundary). v0.10 now uses
   `state { f: T derived [shape: …] }`; the projection lives in
   each substrate's `maps` block. Both the spec (§6.6) and the
   Twitter example were updated accordingly.
2. **Symbolic freshness epochs had no semantics.** v0.10 now requires
   substrates to declare named epochs via `epoch <name> { advances_on
   <action-set> ; covers <field-set> }` (new §7.2.4). Freshness
   slots reference an epoch name; undeclared epochs error.
3. **The obligation pass had no fact schema.** New §15.1.5 normatively
   enumerates the base + derived predicates and states the complexity
   claim (polynomial in source size; transitive closure for
   availability dependencies). Monotonicity is restated carefully
   (derivation is monotone; unhandled-obligation report is not).

Three findings deliberately parked (see §14.1): a formal label
calculus for retention/info-flow, slots-as-effect-rows refactor,
discharge vs acknowledgement separation. v0.10 commits to
**explicit-flow only** for retention checks; implicit flows are
not a violation.

## v0.9

The "boundary completeness" round. Three observations drove it:

1. v0.7 closed every refinement-related gap from R1–R6, but Surface
   still couldn't say "you didn't think about idempotency" or "you
   didn't think about retention." It was a refinement checker, not
   a boundary-completeness compiler.
2. Earlier drafts framed Surface as "a language that compiles to
   TLA+." That understated the frontend — most of Surface's value
   should live in static, fast, local checks before TLA+ ever runs.
3. P1‑7 (actor-relative surface state) was the last remaining
   structural gap. Twitter's `posts`/`follows`/`protected` are
   genuinely not globally observable; the v0.7 spec had no clean
   home for them.

The four moves:

- **`coverage.md` — Coverage Manifest.** A new normative document
  enumerating every dimension Surface covers, which construct addresses
  each, since-version, **and a versioned list of what Surface
  deliberately does not cover**. v0.9 is the first version where
  "what's in scope / what's out" is a first-class artifact rather
  than scattered across the spec.

- **Mandatory action coverage slots (§6.4).** Every `action` must
  carry six slots — `idempotency`, `auth_channel`, `retention`,
  `rate_limit`, `observability`, `availability` — each a closed enum
  with a `waived: "<reason>"` escape. The compiler errors on missing
  slots. Substrates' `authentication { … }` mapping is cross-checked
  against `auth_channel`; emit/log of a `retention: secret` value is
  a static error. The slot pass runs as `surface check --slots`,
  separate from refinement; it is a precondition for codegen.

- **`observable for <Actor>` (§5.3) — closes P1‑7.** Actor-relative
  view projections: `observable for u: User visible_posts(u) : Set[...]
  = …`. The compiler binds the actor variable to the calling action's
  `by` parameter automatically; cross-actor leaks are static errors
  (R-ACTOR-VIEW-LEAK). Twitter's global `posts`/`follows`/`protected`
  state migrates to per-user views in `examples/twitter/`.

- **§15 — Static obligations (sketch).** The framework for a
  consequence-inference pass: derivation rules over declared facts
  produce *obligations* the user must `acknowledged { … }` or
  discharge. v0.9 ships the framework, a closed enum of obligation
  kinds, and six representative rules (R-AVAIL-READ, R-AVAIL-CHANNEL,
  R-TRUST-PARAM-AUTH, R-WRITE-CONFLICT, R-REPLAY-AMP, R-RETENTION-FLOW,
  R-ACTOR-VIEW-LEAK). The full rule catalog is **deliberately deferred
  to v0.10**.

Plus, a positioning change: **TLA+/PlusCal is Surface's primary
backend, not its semantics.** The slot pass and the obligation pass
live in the frontend and produce Rust-style local errors with a fix
site. They run on every save; the TLA+ backend runs on a schedule.
`overview.md` principle #2 and §12 of the spec were rewritten to
reflect this.

**Spec changes** (cross-referenced to the construct/section):

- New §5.3: `observable for <Actor>` (P1‑7).
- New §6.4: Mandatory coverage slots — six closed-enum slots with
  `waived:` escape, compiler-error rules, docs-projection checklist
  rendering, v0.7→v0.9 migration tool sketch.
- New §15: Static obligations sketch — framework + obligation-kind
  enum + `acknowledged { … }` block + 6 representative rules.
- §12 Tooling: separate `--slots` and `--obligations` passes; reframe
  TLA+ as one backend among Surface's outputs; tooling table added.
- §14 Omissions: P1‑7 removed (closed); full §15 rule catalog and
  user-extensible rules added as deliberate v0.9 exclusions.

**Examples updated:**

- `examples/url-shortener/`: actions carry the six slots; some
  realistically waived (rate_limit, availability) given the spec's
  didactic scope.
- `examples/twitter/`: actions carry the six slots; `posts`,
  `follows`, `protected` migrated to `observable for User` views.
- `examples/url-shortener/docs-sample.md`: refreshed to show the
  per-action boundary checklist rendered from slots (the P1‑10
  mitigation).

**Closed**: P1‑7. Partially mitigated: P1‑10 (slot checklist softens
the property-body cliff in the docs projection; spec text remains a
wall for nested quantifiers).

**R7 evaluation**: GPT‑5.5 and Claude Opus 4.7 each independently
authored a v0.9 CloudFront spec as a reviewer exercise. See the R7 row
in the agent feedback log for the convergent findings folded into v0.10.

## v0.7

A consolidation/hygiene round: no headline new constructs, just
clearing the high-priority small-and-medium backlog from `TODO.md`.
Drove from the R6 example audit + language-designer critique.

**Spec changes** (each cross-references the resolved TODO id):

- **Cuts** to reduce surface area (designer review):
  - `system Foo { … }` sugar removed (`module Foo` is the only
    container) — *C-1*.
  - `permission name(args) for u: User when …` declaration form
    removed; use a Boolean `observable` and call it from `when` —
    *C-3*.
  - `aggregate using all` and `aggregate using any` aggregators
    removed (use a comprehension and `choose` respectively) —
    *C-4*.
- **`Returned<X>` semantics pinned** (S0-1, C-2): implicit only, with
  fields `(value: T, to: Actor)`; explicit declarations are illegal.
- **`observed by` is checked semantically** against the event's
  recipient field by name resolution (`to` → `by` → `recipient`) —
  *S0-10*.
- **Cardinality** (`s.size`/`m.size`/`|s|`) added for sets and maps —
  *S0-11*.
- **`Map[K -> V]` is a finite partial map**; total maps via
  comprehension; no new type — *P0-5*.
- **Effect grammar** (P0-7, P0-8, S0-2, S0-4):
  - `aggregate ...` is now legal in action bodies, not only in
    `maps`/`observable`/`history_predicate`.
  - `emit` is exempt from the `for ... do` order-independence
    restriction (events appended in iteration order are atomic per
    surface step).
  - Bounded `choose x in <set>. <pred>` form added.
  - Optional unwrap: `if let Some(x) := opt then ... else ...` and
    the `match opt { Some(x) -> e1; None -> e2 }` value form.
- **Branch coverage rule** (S0-6): every leaf branch label of a
  surface action must be realized somewhere across the project.
- **Multi-disjunct guards without labels** (P1-9): warning
  recommending labels for any non-trivial guard split.
- **Error refinement** (P1-6) — new sub-section showing the two
  blessed patterns: substrate mirrors the raise guard, or substrate
  leaves the action disabled (stutter).
- **`aggregate` typing tightened** (P1-2, P1-3): split `union_set`
  (set semantics, deduplicating) from `concat_seq(order_by ...)`
  (deterministic sequence concatenation); `else <expr>` clause for
  `max`/`min` empty scopes; map-valued example added.
- **Realization parameter binding scope** (S0-7) — new sub-section
  pinning down what's in scope inside `realizes` `when` guards.
- **`cross_visible` is a two-way contract** (P0-6): owning substrate
  declares the invariant; either composed substrate may write.
- **Cross-substrate channel worked example** added (P1-4) including
  the `Component[*].receives.MsgName` form for `internal`/`fairness`.
- **`by stutter` built-in** for compose-level realization of
  no-effect branches (S0-8, C-5). Drops the informal `by no_op` and
  the explicit-`noop`-action workaround.
- **`forbidden` vs. `safety`+`fails with`** clarified in §8 (S0-5):
  asserting a deterministic failure is `safety`, not `forbidden`.
- **Event-log helpers**: bounded forms `last(seq, pred)` /
  `first(seq, pred)` / `count(seq, pred)` added; duplicate-event
  semantics pinned (slicing helpers index by occurrence; `first`
  occurrence for `events_before`, `last` for `events_after`) —
  *S0-3, S0-9*.
- **`state_at(e)` helper** (P1-5): returns a snapshot of surface
  state at the moment `e` was emitted; usable in `history_predicate`
  bodies. The Twitter `is_following_at`/`is_protected_at` predicates
  switched to this and are now genuinely time-relative.
- **`extern` is top-level only** confirmed (S0-12).
- **§14 omissions refreshed** (cross-module liveness deferred to
  v0.8; everything else unchanged).

**Closed bookkeeping** (already done; just marked closed):
P0-1 through P0-4, P1-1.

**Examples updated**:

- `examples/twitter/`: `using union` → `using union_set`; the
  `Index.noop` workaround dropped in favour of `by stutter` in
  `compose.surf`; history predicates rewritten to use `state_at(e)`;
  README and v0.7 feature table updated.
- `examples/url-shortener/`: minor refresh; banner bumped.
- New: **`examples/url-shortener/docs-sample.md`** (P1-8) —
  hand-rendered Markdown showing what `surface emit docs` will
  produce. The "non-technical reader can stop at the surface" claim
  is now falsifiable; the deferred actual generator just needs to
  match this shape.

**TODO.md**: every closed item annotated; remaining backlog is
mostly P1-7 (L), the P3-* deferred items, and any new findings from
the v0.7 verification audit.

## v0.6

Driven by the v0.5 two‑agent CloudFront round. Six convergent findings, all in:

- **`aggregate Component[id].field over <Scope> using <Aggregator>`** —
  first‑class aggregation across replicated instances inside `maps`,
  `observable`, and `history_predicate` bodies. Aggregators: `exists`,
  `forall`, `any`, `all`, `sum`, `max`, `min`, `union`. Eliminates the
  `choose id … exists id …` chains that v0.5 specs needed by hand. Both
  v0.5 agents named this as the #1 missing feature. (§7.2.3)
- **Cross‑substrate `realizes` overrides in `compose`** — a compose block
  may add or strengthen a `realizes` clause whose guard reads the *other*
  substrate's state. The owner declares which auxiliary variables are
  reachable across substrates with `cross_visible`. Closes the
  joint‑refinement gap (every‑edge‑has‑acked, quorum reached, distributed
  commit). (§7.7.3)
- **`sends … to <ChannelName>` call‑site syntax** spelled out for
  cross‑substrate channels declared in `compose`. The channel name is in
  scope inside any component of either composed substrate. (§7.2.1)
- **Type signatures and predicate semantics for the event‑log helpers**
  `events_before`, `events_after`, `first`, `last`, `count`, `between`.
  The predicate form is `e is X && e.field == …`. (§9.1.1)
- **Branch labels (`[hit]`, `[miss]`, …) must be unique within an action
  body**; the order `when` → `raises` → labeled `if/else` is now
  explicit. `raises` always wins over `if/else`. Pairwise overlapping
  raise guards are statically rejected. (§6.1.1)
- **Tuple keys from `cross`** — `A cross B` is `Set[(A, B)]`; tuples are
  valid map keys; comprehensions can destructure with `(a, b)`. (§3.1)

Examples:

- Added `examples/twitter/` exercising `replicate`, `aggregate`,
  `history_predicate`, `attacker`, `e is X`. Single substrate.

## v0.5

Driven by the v0.4 three‑agent CloudFront round. The unanimous v0.4 finding
was that splitting a system into peer substrates (data plane vs. control
plane) was painful because every substrate had to map every surface field.

- **`partial substrate … owns { <fields> }`** — a substrate may declare
  it owns a subset of the surface's state fields; it only needs to map
  those. A plain `substrate` (no `partial`) is shorthand for "owns
  everything." (§7.7.1)
- **`compose <Name> = <Sub1> + <Sub2> + …`** — project‑level block that
  joins partial substrates into a complete realization, declares any
  channels that cross substrate boundaries, and runs three project‑wide
  checks: ownership partition, action coverage, channel completeness.
  (§7.7.2)
- **`replicate` × `channel` scoping** spelled out: `from A to B[*]`
  (broadcast), `from A[*] to B` (fan‑in), `from A[i] to B[i]`
  (pairwise), `from A[*] to B[*]` (N×M with explicit destination).
  (§7.2.1)
- **Labeled `if/else` branches** referenceable from `realizes`.
  `then if cond then [hit] … else [miss] …` on the surface; substrates
  write `surface.get[hit] by Edge[id].serve_hit`. (§6.1)
- **`param.<name>` as an `authentication { … }` RHS.** Actor identity
  may come from an action parameter instead of a long‑lived state field.
  (§7.5)
- **`history_predicate` blocks** for reusable event‑log predicates so
  multi‑quantifier history claims stay readable. (§9.1)
- **`_` wildcards** in event matches; **`e is <EventName>`** event‑type
  test with field access on the narrowed type. (§5.1.1)

Examples:

- `examples/` directory introduced; `examples/url-shortener/` is the
  first worked example. `02-examples.md` removed.

## v0.4

Driven by a hands‑on CloudFront evaluation by Sonnet 4.6 and Opus 4.7
under v0.3. Their #1 finding: no `if/else` in effects.

- **Conditional effects** `if <guard> then <effect> else <effect>` in
  `then` blocks; compiles to two next‑state disjuncts.
- **`let` and `choose` in `then` blocks.** `let x := e` introduces a
  binding; `choose x: T. P(x)` is TLA+ `CHOOSE`.
- **`observable name(args): T = expr`** formalized in §5 (previously
  appeared in `02-examples.md` only).
- **`A cross B`** as a first‑class set‑product operator.
- **`forall` / `exists`** explicitly allowed in `when` and `raises`
  guards (one expression sublanguage everywhere).
- **`by EXTERNAL` in `realizes`** — a substrate may declare that a
  surface action is realized externally; satisfies completeness without
  a refinement obligation.
- **`replicate Component[id in IDS] { … }`** value‑parametric
  components.
- **Fairness disambiguation** for multi‑realized surface actions.
- **`authentication { … }` RHS grammar** formalized: component path or
  `system`.
- **Channels gain a documented `sends`/`receives` form**.

Modularity:

- **A module may be defined across multiple files.** Every `.surf` file
  starts with `module <QualifiedName>`; the compiler unions the
  contributions. `surface check ./project/` is project‑aware.

## v0.3

Driven by two parallel critiques (a language‑design critique and a
formal‑methods critique) of v0.2.

Soundness fixes (formal methods):

- **Auxiliary variables (`history` / `prophecy`)** first‑class in
  `substrate` blocks. Without these, Abadi & Lamport's completeness
  result is violated and many real implementations cannot be proved
  correct against a strong surface.
- **Fairness** part of the language: `fairness weak/strong <action>`
  on surface and substrate.
- **Compositional reasoning restricted to safety** in v0.3; cross‑module
  liveness deferred.
- **Default‑deny for substrate steps:** every substrate action must
  appear in `realizes` or `internal`.
- **`internal` actions emit an explicit `UNCHANGED Map(surface_vars)`
  obligation.**
- **Surface → TLA+, substrate → PlusCal** by default.
- **`attacker` scoped to integrity reachability** (not confidentiality).
- **`authentication { … }` mapping** required in substrates so attacker
  checks aren't surface‑theatre.

Language ergonomics:

- **Unified error model:** `raises { Name when G }`; `fail` removed.
- **`invariant` removed**; one form: `property name { always P }`.
- **Typed `event` declarations** replace `visible_as "string {templates}"`.
- **Actor subtyping** (`actor B extends A`).
- **`extern` for spec parameters** set at check time.
- **Expression grammar formalized** (`+=`/`-=`/`:+`, comprehensions,
  sequence ops, `union`/`intersect`/`diff`).
- **Action return values** added to grammar.
- **Scenarios require an explicit `kind:`** (`safety` / `liveness` /
  `forbidden`).

## v0.2

Decisions locked in after the initial v0.1 sketch:

- **Braces‑only grammar**, whitespace insignificant.
- **Refinement mapping syntax:** `maps { … }` + `realizes { … }` +
  `internal { … }`.
- **`module` as the canonical container**; `system Foo` is sugar for a
  top‑level `module Foo`.
- **TLA+ / PlusCal** as the only codegen target; SDK and clocks
  deferred.

## v0.1

Initial sketch. Headline idea: every system has a *surface* (what users
observe) and a *substrate* (how it's built); a single `.surf` file holds
both, with mechanical refinement between them. One file produces:
formal model, refinement obligations, Gherkin/pytest skeletons, prose
docs, threat model.

---

