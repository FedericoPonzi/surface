---
title: Language Specification
description: Normative spec for Surface v0.10.1 — lexical structure, types, surface/substrate blocks, scenarios, properties, attackers, obligations.
show_table_of_contents: true
---
# Surface — Language Specification (v0.10.1 draft)

This is the normative spec for the Surface language. It is intentionally
small. Anything not expressible in Surface can be written directly inside a
`tla { … }` escape block (subject to §13).

The semantics are given by translation to TLA+ (with PlusCal used as the
target for `substrate` blocks). Where the document refers to "the generated
TLA+", it means the output of `surface emit tla`.

For the elevator pitch and design principles, read [`overview.md`](overview.md)
first. For modular composition, see [`modules.md`](modules.md). For an
audit of what Surface covers vs. deliberately excludes, see
[`coverage.md`](coverage.md).

## Contents

1. [Lexical structure](#1-lexical-structure)
2. [Top-level declarations and multi-file modules](#2-toplevel-declarations-and-multifile-modules)
3. [Types](#3-types)
4. [Actors](#4-actors)
5. [Events and observables](#5-events-and-observables)
6. [The `surface` block](#6-the-surface-block) — state, actions, the seven mandatory slots (§6.4), defaults, derived state
7. [The `substrate` block](#7-the-substrate-block) — components, channels, auxiliary, authentication, maps, realizes, internal, epochs
8. [`scenario` — executable use cases](#8-scenario--executable-use-cases)
9. [`property` — invariants and temporal claims](#9-property--invariants-and-temporal-claims)
10. [`attacker` — security as integrity reachability](#10-attacker--security-as-integrity-reachability)
11. [Documentation extraction](#11-documentation-extraction)
12. [Tooling](#12-tooling)
13. [Escape hatch](#13-escape-hatch)
14. [Notable v0.10 omissions (deliberate)](#14-notable-v010-omissions-deliberate)
15. [Static obligations — the consequence-inference pass](#15-static-obligations--the-consequenceinference-pass-v09-expanded-v010)

---

## 1. Lexical structure

- UTF‑8 source files, extension `.surf`.
- Comments: `--` to end of line, `{- … -}` block.
- Identifiers: `[a-zA-Z_][a-zA-Z_0-9]*`. CamelCase for types, actors,
  events, and modules; snake_case for fields and actions.
- Strings are double‑quoted; triple‑quoted (`"""…"""`) for multi‑line
  doc‑strings.
- **Whitespace is insignificant.** All blocks are delimited by `{ … }`.
  Statements within a block are separated by newlines or `;` (either works).
  A formatter may reflow without semantic change.

## 2. Top‑level declarations and multi‑file modules

```
module <QualifiedName>                       -- header: this file's module

use    <Module>.{ <Name>, ... }              -- import names
type   <Name> = <TypeExpr>
actor  <Name>
actor  <Name> extends <Other>                -- actor subtyping
event  <Name>(<arg>: <Type>, ...)            -- typed observable
const  <Name> : <Type>                       -- model parameter (set in .cfg)
extern <Name> : <Type>                       -- supplied at check time

surface       { … }
substrate         <N> realizes <SurfaceName?> { … }   -- v0.4: full ownership
partial substrate <N> realizes <SurfaceName?> owns { <field>, ... } { … }
                                                       -- v0.5: field ownership
compose <Name> = <Sub1> + <Sub2> + …  { … }            -- v0.5: project-level
                                                       -- composition + shared
                                                       -- channels
scenario <"title"> { … }
property <Name> { … }
history_predicate <Name>(<arg>: <Type>, ...) { … }     -- v0.5: reusable
                                                       -- predicate over the
                                                       -- event log
attacker <Name> { … }
```

### 2.1 One module, many files

Every `.surf` file begins with a `module <QualifiedName>` header and
contributes its declarations to that module. The compiler unions
declarations of the same module across files. There is no penalty for
splitting a module across files; conversely a small module may live in a
single file.

```
project/
  Banking/
    surface.surf            -- module Banking; surface { … }
    sql_substrate.surf      -- module Banking; substrate SqlMonolith { … }
    saga_substrate.surf     -- module Banking; substrate DynamoSaga { … }
    scenarios.surf          -- module Banking; scenarios + properties
    Ledger/
      surface.surf          -- module Banking.Ledger
      memory_substrate.surf -- module Banking.Ledger
```

`surface check ./project/` discovers every `.surf` file under the path,
groups them by their `module` header, and assembles the module graph. The
directory layout is a *convention* for humans; the binding name is the
explicit `module` header.

A module has **at most one** `surface { … }` block across all of its
files (the compiler errors on duplicates), zero or more `substrate`
blocks, and any number of types, actors, events, scenarios, properties,
and attackers.

> **v0.7:** the `system Foo { … }` sugar (legacy from v0.1) was
> **removed**. `module Foo` is the only top‑level container. Any
> existing `system` declaration becomes `module` mechanically.

### 2.2 Visibility

A module may mark a sub‑module `private`:

```
module Banking.Internal private
```

`private` modules are visible only inside the parent. The compiler
rejects any `surface` outside the parent that references a `private`
module's names.

## 3. Types

```
Nat | Int | Bool | String
Set[T] | Seq[T] | Map[K -> V] | Optional[T]
{ field1: T1, field2: T2, ... }     -- records
A | B | C                           -- tagged union
enum { Red, Green, Blue }
```

Every type used in a checked spec must have a finite check‑time domain
(driven by `const`s and `extern`s) so the model checker can enumerate.

### 3.0 `Map[K -> V]` is a finite partial map (v0.7)

`Map[K -> V]` has these primitive forms:

- `{}` is the empty map (no keys defined).
- `m[k]` is the value at `k` if `k in m.keys`; reading an undefined
  key is a *static error* in well‑typed specs (the compiler checks
  `k in m.keys` flows where the read happens) or, when checked
  dynamically, traps at the model checker.
- `m.keys` and `m.values` give the defined keys/values as sets.
- `m[k] := v` defines or updates the value at `k`.
- `delete m[k]` removes the key.
- `k in m.keys` is the explicit presence check.

A **total map over a finite domain** is constructed with a map
comprehension:

```
{ k -> default(k) | k in DOMAIN }
```

This is total over `DOMAIN` by construction; the compiler can prove
that `m[k]` is defined for every `k in DOMAIN`. There is no separate
`TotalMap` type — totality is a property of construction, not a kind
of map.

### 3.1 Set / map operations

```
s union t       s intersect t       s diff t
s.size          m.size                       -- cardinality (Nat); also `|s|`, `|m|`
A cross B                                    -- Cartesian product
{ x in s | P(x) }                            -- set comprehension (filter)
{ f(x) | x in s }                            -- set comprehension (map)
{ k -> f(k) | k in s }                       -- map comprehension
seq.head    seq.tail    seq.length    seq :+ e
forall x in s. P(x)
exists x in s. P(x)
sum(m.values)    sum(s)
```

`size` works on `Set` and `Map` (and `Seq` — synonym for `.length`).
The shorthand `|x|` is equivalent to `x.size`.

A cross product `A cross B` produces a `Set[(A, B)]` whose elements are
tuples. Tuples are valid map keys (`Map[(A, B) -> V]`); index a tuple's
components with `.0`, `.1`, … (zero‑based). For multi‑variable
comprehensions over a cross product the bound variable destructures by
name when written as `(a, b)`:

```
{ (a, b) -> 0 | a in A, b in B }       -- explicit names
{ x.0 + x.1 | x in A cross B }         -- positional access
```

These operations and quantifiers are usable in `when` guards, `raises`
guards, `then` effect bodies, `property` bodies, `maps` mapping
expressions, `observable` definitions, and `history_predicate` bodies —
any boolean / value position. There is no separate "expression
sublanguage for properties" vs. "expression sublanguage for guards";
it is all one expression language.

## 4. Actors

```
actor User
actor Admin extends User           -- Admin is-a User; (u: User) is Admin works.
```

> **v0.7:** the standalone `permission name(args) for u: User when …`
> declaration form was **removed** (it was redundant with named
> Boolean predicates and pure observables). To express a reusable
> authorization predicate, define a Boolean `observable`:
>
> ```
> observable can_view(a: Account, u: User) : Bool =
>   owner[a] == u || u is Admin
>
> action read(a: Account) by u: User
>   when can_view(a, u)
>   …
> ```
>
> **v0.7:** the `attacker … may any action allowed for snoop` form
> (§10) resolves against the actions whose `when` clause holds for
> the attacker, so the security story is unchanged.

## 5. Events and observables

### 5.1 Events

Events are user‑observable side effects.

```
event Disclosed(d: DocId, body: String)
event Redirected(target: Url)
```

Events are first‑class. The surface block has an implicit state variable
`events: Seq[Event]` initialized to `<>`. The `emit E(...)` effect appends
to it. Scenarios assert membership using `observed E(arg=…) by <actor>`.

The `events` log is a *sequence*, not a set: helpers like
`events_before(e)` (§9.1.1) reason in terms of the **occurrence index**
of `e` in the sequence. If the same event value appears twice
(possible since events are equal-by-value records), `events_before(e)`
uses the **first** occurrence; `events_after(e)` uses the **last**.
Authors who need to disambiguate should add a serial field
(`event Foo(... seq: Nat)`).

#### 5.1.1 Return values and the implicit `Returned<X>` event (v0.7)

A surface action declared with a return type:

```
action visit(s: Slug) -> Url
  by    v: Visitor
  …
  then  …
        return links[s]
```

desugars `return e` to:

```
emit Returned<Visit>(value=e, to=v)
```

where `Returned<Visit>` is an **implicit** event with the fixed shape
`(value: <ReturnType>, to: <ActorType>)`. Authors **must not** declare
`event Returned<X>(...)` themselves — the compiler reserves these
names. Substrates whose realizing action also has a return type emit
the same implicit event automatically; refinement holds when the
returned values match.

Scenarios may assert on these events directly:

```
observed Returned<Visit>(value="https://example.com", to=V) by V
```

#### 5.1.2 `observed E(...) by X` recipient resolution (v0.7)

The `by X` clause in a scenario `observed` assertion is **checked
semantically** against the event's recipient field by name resolution:
the compiler picks the first of `to`, `by`, or `recipient` that
appears in the event's declaration, and asserts that field equals
`X`. If the event has none of those fields, `by X` is a syntax error.
This catches the silent failure where authors assert `by V` on an
event whose recipient field is named differently.

#### 5.1.3 Wildcards and event‑type tests

In any position that matches an event (scenario `observed`, property body,
`history_predicate` body), the underscore `_` matches any value:

```
observed Served(d="D1", p=_, body=_, cache_hit=_, by=V) by V
```

Within an expression, `e is <EventName>` is true iff `e` was constructed
with that event constructor:

```
exists e in events. e is Served && e.cache_hit
```

`e.field` works on the matched event after `e is X` has narrowed its
type. The compiler rejects field accesses on events whose type has not
been narrowed.

### 5.2 Observables

```
observable <name>(<arg>: <Type>, ...) : <Type> = <pure expr over surface state>
```

An `observable` is a pure derived view of surface state. It has no
effects. It may take parameters. Scenarios may assert on observables
directly (`then resolves("go") == Some(...)`). Substrates inherit
observables for free — they are computed from the projected surface state
via `maps`.

Observables are *not* their own state variables; they are macros expanded
inline at every reference site in the generated TLA+. The compiler rejects
mutually recursive observables and observables that reference substrate
state.

### 5.3 Actor-relative observables — `observable for <Actor>` (v0.9)

Some surface state is genuinely **not globally observable**: every user sees
a different projection of it. Twitter's `posts`, `follows`, and `protected`
are the canonical examples — there is no single user who can observe the
"global" follow graph, only their own outgoing edges and the incoming
edges of accounts they themselves can see. v0.7 forced authors to either
expose the global state (false: no actor can read it) or hide it behind a
parameterised `observable` (loses the refinement story, because the
substrate-side projection is then ambiguous).

v0.9 introduces actor-relative observables:

```
observable for <var>: <Actor> <name>(<arg>: <Type>, ...) : <Type> =
  <pure expr over surface state and <var>>
```

The `for <var>: <Actor>` clause binds the **observing actor**. The body
may reference both that actor and any surface state. The compiler's rules:

1. **Calling-actor binding.** When the observable is referenced inside an
   `action`'s `when`, `raises`, or `then`, the `<var>` is bound to the
   action's `by <Actor>` parameter automatically. References from a
   `scenario` or a `property` must supply the actor explicitly:
   `protected_for(u=Alice)`.
2. **Refinement obligation per actor.** A substrate's `maps` block must
   define a projection of substrate state to the observable, parameterised
   by the same actor. The refinement obligation is universally quantified
   over the actor: for every `u: User`, the substrate's projection at `u`
   must agree with the surface's value at `u`.
3. **No actor leakage.** The body of an `observable for u: User` may not
   project values that depend on actors other than `u` unless those
   actors are themselves reachable from `u` via state the spec exposes
   (e.g. "the set of accounts `u` follows"). The compiler enforces this
   by tracking which actor variables a body's reads depend on; a
   dependency on a free actor variable that is not derivable from `u`
   is a type error.

Worked example (Twitter):

```
observable for u: User visible_posts(u) : Set[(User, Nat)] =
  { (author, n) | (author, n) in all_post_ids
                , author == u
                  || (follows[(u, author)] && not protected[author])
                  || (follows[(u, author)] && approved_followers[author].has(u)) }

observable for u: User can_follow(target: User) : Bool =
  u != target
  && not blocks[(target, u)]
  && (not protected[target] || approved_followers[target].has(u))

action post(content: String) by u: User
  -- when no explicit args, the actor-relative observables are bound to u
  then  ...
```

In a scenario:

```
scenario "non-follower cannot see protected tweets" kind: forbidden {
  actors { Alice: User, Snoop: User }
  given protected[Alice] == true
        not follows[(Snoop, Alice)]
        all_post_ids == { (Alice, 1) }
  -- Note the explicit actor binding:
  then  (Alice, 1) in visible_posts(u=Snoop)
}
```

The implicit form **subsumes** the parameterised-observable workaround
from v0.7 (`is_following_at(viewer, target, e)` style) for the common
case where the observing actor *is* the action caller. The
parameterised form remains valid for predicates that genuinely take
two unrelated actors as arguments (e.g. "does A follow B?", asked
from a global perspective).

> **v0.9 closes P1‑7.** Authors should migrate global "actor-relative"
> state to `observable for` views and keep only the truly global state
> in `state { … }`. The v0.9 `examples/twitter/` migration is the
> canonical reference.

#### 5.3.1 Rule 3 clarification — globally-keyed non-actor state (v0.10)

The "no actor leakage" rule (rule 3 above) constrains references to
*actor variables* in the body of `observable for u: <Actor>`. It does
**not** constrain references to state indexed by non-actor keys.

```
-- Legal: cache_behavior is Map[DistId -> CacheBehavior]; the key is
-- a DistId, not an actor. Reading it from inside `observable for v` is fine.
observable for v: Viewer is_blocked_for(d: DistId) : Bool =
  cache_behavior[d].geo_blocklist.contains(v.country)
```

The rule still forbids referencing a free actor variable that is not
derivable from `u`:

```
-- ILLEGAL: `attacker` is a free actor variable not derivable from `v`.
-- Static error: E_ACTOR_VIEW_LEAK.
observable for v: Viewer can_see_other(attacker: Viewer) : Bool =
  follows[(attacker, v)]
```

The compiler's check is: every free *actor-typed* variable other than
the for-bound `u` must be either an argument of the observable
(typed as a non-actor index) or reachable from `u` through state.

#### 5.3.2 Auto-binding in substrate `realizes when …` (v0.10)

Rule 1 originally specified auto-binding for `observable for u: <Actor>`
references inside the action's own `when`, `raises`, and `then` blocks
on the surface side. v0.10 extends this to **substrate-side `realizes
when …` clauses**: when a substrate's `realizes` clause references an
actor-relative observable, the `u` is bound to the realising substrate
action's actor-resolution from `authentication { … }`.

```
-- Surface side: observable bound to the calling Viewer automatically.
action get(d: DistId, p: Path) -> Optional[Body] by v: Viewer
  …
  when not is_blocked_for(d)        -- `v` auto-bound here (rule 1)
  …

-- Substrate side: `is_blocked_for(d)` in the realizes guard is bound
-- to the actor that authentication resolves for Edge[id].serve.
realizes {
  surface.get(d, p) by Edge[id].serve(d, p)
    when not is_blocked_for(d)      -- v0.10: auto-bound to authentication
                                     -- resolution of Edge[id].serve
}
```

For explicit control (e.g. when a compose-level `realizes` reads an
observable for a non-calling actor), pass the actor argument explicitly:
`is_blocked_for(d, v=param.v)`. The compiler accepts either form;
auto-binding is the default, explicit binding is the override.

## 6. The `surface` block

```
surface {
  state { <field>: <Type> [ retention: <RetentionValue> ] [ private ]
          ...
          <field>: <Type> derived [ shape: <Shape> [ of: <Type> ] ]   -- v0.10
          ... }

  defaults { <slot> : <SlotValue> ... }   -- v0.10: §6.4.5

  init { <field> := <expr> ... }   -- may reference `extern`s and `const`s

  fairness weak <action_name>
  fairness strong <action_name>

  property <name> { always <P> }       -- safety
  property <name> { eventually <P> }   -- liveness (requires fairness)

  action <name>(<arg>: <Type>, ...) [-> <ReturnType>]
    by    <var>: <Actor>
    when  <Pre>                            -- positive guard
    raises { <ErrName> when <RaiseGuard>, ... }
    <mandatory-slots>                      -- §6.4 (seven slots, v0.10)
    then  <Effect>                         -- atomic state update + emits

  internal_action <name>(...) [-> ...] ... -- v0.10: §6.4.6, auto-fills 4 slots
    by    sys: <ActorClass>
    ...
    then  <Effect>
}
```

### 6.1 Effect language

```
x := e                  -- assignment
x += e   x -= e         -- compound (Nat / Int)
m[k] := v               -- map update
delete m[k]
s := s union {e}        -- set union; intersect, diff also keywords
seq :+ e                -- snoc (append to seq)
emit E(arg=v, ...)      -- append to events
return e                -- desugared into an implicit Returned<Name> event

let <name> := <expr>    -- bind a name for the rest of the block
                        -- (let-bindings shadow but never mutate state)

if <guard> then         -- conditional effect; compiles to two next-state
  <effect_block>        -- disjuncts with negated guards. The two branches
else                    -- need not assign the same variables, but variables
  <effect_block>        -- left untouched in a branch are UNCHANGED there.

if <guard> then [<branch_label>]   -- labeled branches (v0.5). The label is
  <effect_block>                   -- a name for this disjunct; substrates
else [<branch_label>]              -- can target it from `realizes`:
  <effect_block>                   --     surface.<action>[<branch_label>] by ...
                                   -- Eliminates the duplication that arises
                                   -- when each substrate has to re-state the
                                   -- guard from the surface action.

choose <name>: <Type>. <Predicate>
                        -- TLA+ CHOOSE: picks any value satisfying the
                        -- predicate. Use inside a `let`. Deterministic
                        -- given the predicate (Hilbert-epsilon style).

choose <name> in <SetExpr>. <Predicate>      -- v0.7: bounded form. Picks
                        -- any element of the set satisfying P. Equivalent
                        -- to `choose <name>: <ElementType>. <name> in <SetExpr> && P`.
                        -- More common in practice; preferred when the
                        -- domain is a value, not a type.

if let Some(<name>) := <expr> then           -- v0.7: Optional unwrap as an
  <effect_block>                             -- effect statement. The body
else                                         -- runs only when the expression
  <effect_block>                             -- is `Some(...)`; `<name>` is
                                             -- bound to the inner value.

match <expr> {                                -- v0.7: value-form Optional
  Some(<name>) -> <expr> ;                    -- (or tagged-union) destructure;
  None         -> <expr>                      -- usable in any value position.
}                                             -- Each arm's expression must
                                             -- have the same type.

for <name> in <SetExpr> do <effect_block>
                        -- bounded iteration over a finite set. The
                        -- iteration order is unobservable for state
                        -- updates whose effect on the same variable does
                        -- not depend on order (compiler statically
                        -- rejects bodies with order-dependent state
                        -- effects, e.g. a `seq :+ x` on a regular
                        -- substrate field). EXCEPTION (v0.7): `emit E(...)`
                        -- inside `for` is allowed — the resulting events
                        -- are appended in iteration order, which is
                        -- atomic at the surface (the whole `then` block
                        -- is one TLA+ step). Authors who care about a
                        -- specific event order inside one action are
                        -- expected to expand the loop manually.

aggregate <Component>[<id>].<expr>            -- v0.7: aggregate expressions
  [ over <ScopeExpr> ]                       -- (defined in §7.2.3) are also
  using <Aggregator>                          -- legal as value expressions
                                             -- in action bodies, not only
                                             -- in `maps`/`observable`/
                                             -- `history_predicate`. Useful
                                             -- inside `let` to compute a
                                             -- value the action then emits.
```

Effects within a single `then` block execute as one atomic surface step.
The block compiles to one TLA+ next‑state disjunct (or several, if the
block contains `if/else` — one per leaf branch). Order of statements
within a branch is the order of effect.

#### Worked example: cache hit / miss

```
action get(d: DistId, p: Path) -> Body
  by    v: Viewer
  raises { NotFound when not (p in published_paths[d]) }
  then
    if cache_valid[(d, p)] then
      emit Served(d=d, p=p, body=cache[(d, p)], cache_hit=true, by=v)
      return cache[(d, p)]
    else
      let body := published_body[(d, p)]
      cache[(d, p)] := body
      cache_valid[(d, p)] := true
      emit Served(d=d, p=p, body=body, cache_hit=false, by=v)
      return body
```

Compiled TLA+ (sketch):

```
get_hit(d, p, v)  ==  /\ p \in published_paths[d]
                      /\ cache_valid[<<d, p>>]
                      /\ events' = events \o <<Served(...)>>
                      /\ UNCHANGED <<cache, cache_valid, …>>

get_miss(d, p, v) ==  /\ p \in published_paths[d]
                      /\ ~cache_valid[<<d, p>>]
                      /\ cache' = [cache EXCEPT ![<<d, p>>] = published_body[<<d, p>>]]
                      /\ cache_valid' = [cache_valid EXCEPT ![<<d, p>>] = TRUE]
                      /\ events' = events \o <<Served(...)>>
```

Both hit and miss are the same *surface action* `get`; the user never
sees a hit/miss distinction in the API. The `cache_hit` flag in the
`Served` event is observable to scenarios but does not split the action.

#### 6.1.1 Branch labels and evaluation order (v0.6)

Branch labels (`[hit]`, `[miss]`, …) on `if/else` arms must be unique
within a single action body. The compiler errors on duplicates. They
are visible only to the action's substrate `realizes` clauses (§7.2);
they do not pollute any wider namespace.

The full evaluation order for any surface action call is:

1. The `when` precondition (§6) is evaluated.
2. The `raises` guards (§6.2) are evaluated. **If any holds, the action
   fires its failure path** with that error name. `raises` guards are
   pairwise independent: at most one may hold at a time, and the
   compiler statically rejects overlapping raise guards.
3. Otherwise the body's labeled branches are evaluated in source order.
   For an `if/else`, exactly one branch fires; for nested `if/else`,
   the nesting order is the source order.

A consequence: `raises` always wins over `if/else`. If you want a
recoverable branch instead of a failure, put the condition inside an
`if/else` body. If you want an unrecoverable failure, declare it as a
`raises`. The two never overlap.

#### 6.1.2 Branch coverage rule (v0.7)

Every leaf branch label of a surface action must be **realized** by at
least one `realizes` clause across the union of all substrates that
realize the surface (per §7.3 / §7.7). The compiler errors at project
load if a labeled branch has no realization. For branches with no
observable effect, use `by stutter` (§7.7.5).

This is a *project-level* check: it is fine for one substrate to
realize `[hit]` and a sibling substrate to realize `[miss]`, as long
as both labels are covered somewhere. (Compose-level realizes
overrides count.)

#### 6.1.3 Multi-disjunct guards without labels (v0.7)

Any surface action with a non-trivial `if/else` (or nested `if/else`)
should label its leaf branches. An unlabelled multi-disjunct surface
action requires every realizing substrate to **re-state the guard** in
its own `realizes when …` — exactly the duplication the v0.5 label
mechanism was designed to eliminate. The compiler emits a *warning*
when an unlabelled action has more than one disjunct on the surface;
labels are not strictly required but are strongly recommended.

### 6.2 The error model

A `raises { Name when G }` clause means: when `G` holds, calling the action
deterministically fails with `Name`, no state changes other than
`emit Failed(action=…, error=Name)` (an implicit event). The action's
"happy path" is enabled iff every `when` predicate is true *and* every
`raises` guard is false.

Equivalent TLA+ (sketch):

```
action_happy ==
   /\ when_pre
   /\ ~raise1_guard /\ ~raise2_guard
   /\ then_effect

action_fails(Name) ==
   /\ when_pre
   /\ raise_guard_for(Name)
   /\ events' = events \o <<Failed(name=Name)>>
   /\ UNCHANGED other_vars
```

Scenarios assert with `fails with <Name>`.

#### 6.2.1 Error refinement (v0.7)

A substrate must **also** realize each surface failure. Two patterns
are blessed:

- **Mirror the guard.** The substrate has its own `raises { Name when G' }`
  clause where `G'` is the projection of the surface's `G` through the
  `maps` block. The implicit `Failed(action, error=Name)` event lines
  up by name on both sides, so refinement holds.
- **Disable the action.** The substrate's realizing action has a
  `when` precondition that goes **false** exactly when the surface's
  raise guard goes true. From the surface's POV the action is
  unavailable; the surface fires the failure path; the substrate
  takes no step (a stutter). Refinement holds because the surface's
  `Failed` event is a state change the substrate doesn't observe — it
  is itself part of the surface contract.

Authors should pick *one* per failure: don't mix patterns for the
same `raises` clause across substrates, or the implicit `Failed`
event may be emitted twice.

The compiler checks the chosen pattern by inspecting which substrate
clauses match each surface `raises` guard.

### 6.3 Fairness

`fairness weak A` adds `WF_vars(A)` to the generated `Spec`. `fairness
strong A` adds `SF_vars(A)`. `eventually` properties are *unsound* without
appropriate fairness — the compiler emits a warning if an `eventually`
property has no fairness annotation that could enable it.

Surface fairness and substrate fairness are **independent**. A substrate
typically needs more fairness (retry actions, message delivery) than the
surface, since the surface is what we are refining *to*.

### 6.4 Mandatory coverage slots (v0.9; refined v0.10)

Every **surface** `action` declaration must carry a **fixed set of
coverage slots** (v0.10: seven slots). Each slot answers one question
about the action's boundary; each slot has a **closed enum of legal
values**; and every slot has an explicit `waived: "<reason>"` escape
for cases where the dimension truly does not apply. The compiler
errors on missing slots (per slot, per action) with the same severity
as a syntax error.

This is the Rust-style "non-exhaustive match" of system specification:
the language enumerates the dimensions, the user must address each one.

> **v0.10 — slots apply to surface actions only.** Substrate component
> `action` declarations (inside `component { … }` / `replicate { … }`)
> are *not* slot-bearing. The previous wording in §6.4 ("every action
> declaration") was ambiguous; v0.10 pins it: slots attach to actions
> declared inside the `surface { … }` block (and to `internal_action`,
> §6.4.6). Substrate authors do not write slots; the substrate inherits
> obligations from the surface slot via the realisation mapping.
>
> Compiler errors are renamed: `E_SURFACE_SLOT_MISSING`,
> `E_SURFACE_SLOT_UNKNOWN_VALUE`, `E_SURFACE_SLOT_ORDER`,
> `E_SURFACE_SLOT_WAIVER_EMPTY`.

Grammar (v0.10 — seven slots; `freshness` added):

```
action <name>(<args>) [-> <T>]
  by      <var>: <Actor>
  when    <Pre>
  raises  { … }
  idempotency   : <IdempotencyValue>
  auth_channel  : <AuthChannelValue>                  -- v0.10: a SET of channels
  retention     : <RetentionValue>
  rate_limit    : <RateLimitValue>
  observability : <ObservabilityValue>
  availability  : <AvailabilityValue>
  freshness     : <FreshnessValue>                    -- v0.10: new slot
  then    <Effect>
```

The slot block appears **after `raises` and before `then`** (or
immediately after `by`/`when` if `raises` is absent). Slot order
within the block is fixed (it is the order the docs projection uses);
the compiler errors on out-of-order slots so authors and reviewers
read the same order every time.

#### 6.4.1 The closed enums

##### `idempotency`

```
idempotency: idempotent                              -- replay is a no-op
idempotency: idempotent by(<arg>, <arg>, …)         -- idempotent keyed by these args
idempotency: at_most_once                            -- replay duplicates the effect
idempotency: at_least_once                           -- caller must tolerate duplicates;
                                                      -- effect may fire >1 time per call
idempotency: waived: "<reason>"
```

The `by(...)` form names a deduplication key explicitly. The compiler
checks that the named args are all parameters of the action.
`idempotent` without `by(...)` means idempotent with respect to the
full argument tuple (the common case).

##### `auth_channel`

How the action's actor identity (`by <Actor>`) gets to this action:

```
auth_channel: session                                -- long-lived authenticated session
auth_channel: bearer_token                           -- single-use or short-lived token
auth_channel: signed_request                         -- per-request signature
auth_channel: capability_url                         -- bearer-in-URL capability
auth_channel: mtls
auth_channel: trusted_caller                         -- internal-only; caller is another
                                                      -- service in the same trust zone
auth_channel: anonymous                              -- no auth; actor is symbolic
auth_channel: { <c1>, <c2>, … }                       -- v0.10: SET form — the action
                                                      -- accepts ANY of these channels.
                                                      -- A bare value `c` is sugar for `{c}`.
auth_channel: waived: "<reason>"
```

> **v0.10 — multi-channel actions and desugaring.** Real systems
> often accept more than one auth channel per endpoint (e.g.
> CloudFront `get` accepts `anonymous` for public objects,
> `signed_request` for signed URLs, `bearer_token` for OAI/OAC). The
> set form lets one surface action cover all of them.
>
> **Desugaring (normative).** `auth_channel: { c1, c2, …, ck }`
> desugars to `k` implicit channel-branches at the **outermost**
> level of the action: every `realizes` clause must select a channel
> (`surface.A[<channel>] by Sub.action when …`). Channel branches
> compose with ordinary branch labels via *nesting*: a body branch
> `[hit]` under channel `signed_request` is `surface.A[signed_request][hit]`.
> Ordering: channel labels come first, body labels come second. This
> rule gives the §6.1.2 branch-coverage check a single canonical
> path through the realisations.
>
> The substrate's `authentication { … }` mapping must produce an
> actor identity consistent with the **specific** channel branch
> being realised; mixed channels in a single substrate require
> distinct realising actions (one per channel).
>
> Single-channel actions remain canonical; the set form is opt-in
> for multi-channel endpoints.

> **v0.10.1 — channel-agnostic `[*]` realises form.** The R8
> evaluation (unanimous, blocking) flagged that the v0.10 §6.4.1
> desugaring "k channel-branches × every body label" forces the
> substrate to write `k × |body labels|` near-identical `realizes`
> clauses when the substrate genuinely does not differentiate
> between channels at the data-plane level. v0.10.1 adds a
> **channel-agnostic realises form**:
>
> ```
> -- Surface side, unchanged:
> action get(d, p) -> Body by v: Viewer
>   auth_channel: { anonymous, signed_request, bearer_token }
>   then if cache_valid[d, p] then [hit] … else [miss] …
>
> -- Substrate side:
> realizes {
>   -- v0.10.1: ONE clause covers every channel branch.
>   surface.get(d, p)[*][hit]  by Edge[id].serve_hit  when …
>   surface.get(d, p)[*][miss] by Edge[id].serve_miss when …
> }
> ```
>
> The `[*]` channel selector means "every channel branch the surface
> declared." It is legal in any `realizes` clause and combines with
> body labels by nesting (channel selector first, body label second,
> same as the per-channel form).
>
> **Compiler rules**:
>
> 1. `surface.<A>(...)[*][<label>]` covers every channel × `<label>`
>    pair declared by `<A>`'s `auth_channel` set.
> 2. `surface.<A>(...)[*]` (no body label) covers every channel
>    branch for an action with no labelled body.
> 3. `[*]` and explicit `[<channel>]` clauses may coexist for the
>    same action; the explicit clauses take precedence (per-channel
>    specialisation). A `[*]` then becomes "every channel not
>    otherwise covered". Compiler emits a warning if `[*]` covers
>    zero channels (all already specialised).
> 4. The substrate's `authentication { … }` mapping for a `[*]`
>    realises must be valid for **every** channel in the set —
>    typically this means `surface_actor of … = param.<x>` where the
>    parameter is consistent with all channels (e.g. an authenticated
>    user identifier produced by an L7 entry that handles channel
>    dispatch before this substrate sees the request).
> 5. The §6.1.2 branch coverage rule is updated: a `[*]` realises
>    satisfies coverage for every channel branch of the action it
>    targets.
>
> **Why this is the right shape.** R8 surfaced that real systems
> often have a single substrate-side data-plane action that doesn't
> care which channel the request arrived on — the channel was
> validated upstream. v0.10's "k channel × |body| realises"
> forced authors to repeat near-identical clauses; the only signal
> reviewers got was that the clauses *were* identical. v0.10.1's
> `[*]` makes the design intent explicit: "this substrate is
> channel-agnostic for this action." Substrates that *do*
> differentiate (e.g. signed-URL verification happens in the
> data plane) continue to use per-channel clauses; nothing about
> the v0.10 form is rescinded.

The substrate's `authentication { … }` mapping must be **consistent**
with this slot: e.g. a single-channel `auth_channel: session` cannot
have `surface_actor of … = param.<argname>` unless the parameter
itself is a session-derived value, and `auth_channel: anonymous`
cannot have a non-trivial `surface_actor` mapping. For the set form,
the compiler checks each channel's authentication path is plausible;
mixed `anonymous` + non-anonymous in one set requires explicit
per-channel branches in the substrate.

##### `retention`

The class of data this action reads, writes, or emits, and how long it
is retained:

```
retention: ephemeral                                 -- not durably stored beyond the request
retention: transactional                             -- durable; user-deletable
retention: audit(period=<DurationConst>)             -- durable; non-deletable until period
retention: pii(class=<PiiClass>, ttl=<DurationConst>)
retention: secret                                    -- credentials, tokens; never logged
retention: waived: "<reason>"
```

`<PiiClass>` is a closed enum: `name | email | phone | address |
biometric | financial | health | location | identifier | other`.
`<DurationConst>` is a `const` of type `Duration` declared at module
level.

The compiler checks any logged/emitted field whose value crosses a
`retention: secret` boundary; emit/log of a secret-class value is a
static error. **For this check to be realisable, fields that hold
secret-class values must be declared `retention: secret` on the state
side** — see §6.5 (v0.10) for state-field retention annotations.

##### `rate_limit`

```
rate_limit: per_actor(<n>: Nat, per: <DurationConst>)
rate_limit: per_target(<arg>, <n>: Nat, per: <DurationConst>)
rate_limit: global(<n>: Nat, per: <DurationConst>)
rate_limit: unlimited                                 -- explicit "no limit"
rate_limit: waived: "<reason>"
```

`unlimited` and `waived` are different: `unlimited` is the design
choice "this action has no rate limit"; `waived` is the design
choice "this dimension is not relevant for this action." The docs
projection renders them differently and reviewers can audit each.

##### `observability`

Which actors can observe that this action happened, and through which
event(s):

```
observability: caller_only(<EventName>, …)
observability: target(<arg>, <EventName>, …)
observability: broadcast(<EventName>, …)
observability: silent                                 -- emits no events
observability: waived: "<reason>"
```

The compiler cross-checks the named events against the action's `then`
block's `emit` statements: every named event must be emitted on at
least one path, and every `emit` in the body must appear in some
slot value. Mismatches are a static error.

##### `availability`

The action's expected availability class. Substrates' real availability
is bounded by their dependencies (see §15); this slot declares the
target:

```
availability: critical                                -- core path; downtime is incident-level
availability: best_effort                             -- degrades gracefully
availability: maintenance_window                      -- may be unavailable during ops
availability: read_only_failover                      -- writes may fail; reads must not
availability: waived: "<reason>"
```

The closure of dependencies derived by the §15 obligation pass must
**not be worse than** this declared slot. The rule
`R-AVAIL-CONSISTENCY` (§15.4) checks: if any substrate this action
transitively depends on declares a weaker availability, the compiler
warns (or errors under `--obligations=strict`).

##### `freshness` (v0.10; epoch semantics pinned per PL review)

For surface actions whose result depends on cached, replicated, or
asynchronously-propagated state, the freshness slot says **how stale
the answer may legally be**, in symbolic terms (no real clocks). The
slot values use **named epochs** declared at the substrate level
(§7.2.4); a `bounded(...)` without a named epoch is illegal.

```
freshness: strong                                     -- linearizable; the action sees
                                                       -- every prior committed write.
freshness: bounded(<epoch_name>, n=<k>: Nat)          -- at most `k` epochs of <epoch_name>
                                                       -- behind. The named epoch must be
                                                       -- declared by the realising
                                                       -- substrate (§7.2.4).
freshness: eventual(<epoch_name>)                      -- eventually consistent under
                                                       -- fairness of <epoch_name>. Without
                                                       -- the epoch name, falls back to
                                                       -- "any propagation epoch declared
                                                       -- by any realising substrate."
freshness: stale_while_revalidate(<epoch_name>, n=<k>: Nat)
                                                      -- stale answer permitted while a
                                                       -- background refresh is in flight;
                                                       -- bounded by `k` epochs of
                                                       -- <epoch_name>.
freshness: not_applicable                              -- the action writes (no
                                                       -- consistency-of-read story).
freshness: waived: "<reason>"
```

> **v0.10 — epoch semantics.** An "epoch" is **not** an undefined
> step count. Substrates declare named epochs via `epoch <name> {
> advances_on <action-set> ; covers <field-set> }` (§7.2.4). The
> obligation pass binds a freshness slot's `<epoch_name>` to a
> matching declaration; unbound references are
> `E_FRESHNESS_UNDECLARED_EPOCH(<action>, <epoch_name>)`.
>
> This avoids the PL-review finding that "the same `bounded(epochs=3)`
> means different things in different substrates": now every freshness
> obligation names the epoch it counts in, and the substrate's
> declaration says which substrate steps advance that epoch.

`freshness` is the right home for the CDN-shaped time/SLA story
(R7's unanimous P1-V9-3). It is **symbolic**: an epoch is counted in
model-checker substrate steps that match the epoch's `advances_on`
clause, not wall-clock time. Specs that need real-time bounds can
use a `const TICK : Duration` and a documentation-only relationship
— the model checker still reasons about epochs.

The freshness slot interacts with the obligation pass: an action
declared `freshness: strong` realised through a substrate channel
declared as fan-out without acknowledgement is an obligation
(`R-FRESHNESS-CHANNEL`, §15.4).

#### 6.4.2 Compiler checks (normative, v0.10)

For every **surface** `action` declaration (see §6.4 preamble — this
applies to surface actions, not substrate component actions):

1. All seven slots are present. Missing slot → error
   `E_SURFACE_SLOT_MISSING(<action>, <slot>)`.
2. Slot values are well-typed against the closed enum. Unknown values →
   error `E_SURFACE_SLOT_UNKNOWN_VALUE(<action>, <slot>, <value>)`.
3. Slot block appears after `raises` (or after `when` if `raises` is
   absent) and before `then`, with the canonical slot order. Out of
   order → error `E_SURFACE_SLOT_ORDER(<action>, <expected_slot>)`.
4. Cross-slot consistency rules:
   - `auth_channel` (set form) vs. `authentication { … }`: at least
     one channel of the set must have a plausible authentication path.
   - `retention: secret` vs. event emits: no event field whose value
     traces back to a `retention: secret` state field may be `emit`ted.
   - `freshness: strong` actions cannot read fields declared
     `derived from` an asynchronous projection (§6.6) unless the
     deriving substrate has a strong-consistency mode.
5. Every `waived: "<reason>"` carries a non-empty reason. Empty
   reason → error `E_SURFACE_SLOT_WAIVER_EMPTY(<action>, <slot>)`.

These checks run in `surface check --slots`, which is a **separate
pass** from refinement checking. A spec with slot errors does not
generate TLA+; the slot pass is a precondition for codegen.

#### 6.4.3 Docs projection

`surface emit docs` renders slots as a per-action boundary
**checklist**, one line per slot, ordered as above. This is the
intended P1‑10 mitigation: the property bodies remain TLA-shaped,
but a non-technical reader gets a complete one-page boundary view
of each action from the slots alone. The url-shortener
`docs-sample.md` is the canonical example.

#### 6.4.4 Migration from v0.7 / v0.9

A v0.7 spec compiled under v0.10 errors at every action with
`E_SURFACE_SLOT_MISSING`. The `surface migrate <from> <to>` tool
inserts `waived: "<from> migration; review"` for every missing slot;
authors then walk the file and replace waivers with real values.
Specs that deliberately want to keep slots waived may leave them so —
the reviewer sees explicit waivers in source rather than silent
absence.

v0.9 → v0.10 specifically:
- Adds `freshness:` to every action (one new slot — the migrator
  defaults it to `waived: "v0.9 migration; review"`).
- Lifts `auth_channel:` enum value to a one-element set
  automatically; no source change needed for single-channel actions.
- Renames slot error codes to `E_SURFACE_SLOT_*`.

#### 6.4.5 `defaults { … }` block (v0.10)

The slot pass is genuinely useful, but six (now seven) lines per
action is real ceremony when most actions in a surface share the
same auth / retention / availability shape. v0.10 adds a top-level
`defaults` block inside `surface { … }`:

```
surface {
  defaults {
    auth_channel  : session
    retention     : transactional
    rate_limit    : per_actor(1000, per=MINUTE)
    availability  : critical
    freshness     : strong
  }

  state { … }

  action create(…)
    by    o: Owner
    -- idempotency and observability still required: not in defaults
    idempotency   : idempotent by(slug, o)
    observability : caller_only(Created)
    then  …

  action visit(…)
    by    v: Visitor
    -- Override the auth_channel default; the rest inherit:
    auth_channel  : { anonymous, capability_url }
    idempotency   : idempotent
    retention     : ephemeral
    observability : caller_only(Visited)
    freshness     : eventual
    then  …
}
```

Rules:

1. `defaults { … }` is a flat block of slot assignments, one or more
   slots. It may set any subset of the seven slots.
2. Each action **explicitly assigns** the slots not covered by
   `defaults`. Slots covered by `defaults` may be omitted; an action
   may *override* a defaulted slot by writing the slot line.
3. The defaulted slot still renders in the docs projection per
   action, with a `(inherited)` annotation.
4. The compiler still errors with `E_SURFACE_SLOT_MISSING` for any
   slot that is neither defaulted nor per-action specified. There
   is no "implicit waiver" via omission.
5. A surface may have **at most one** `defaults` block.

`defaults` reduces ceremony without losing boundary visibility:
every action's effective slot value is fully determined and
fully renderable.

#### 6.4.6 `internal_action { … }` modifier (v0.10)

Surface actions that model control-plane housekeeping (cache
eviction, reconciliation loops, scheduled rebalancing) typically
have a fixed shape:

- `auth_channel: trusted_caller` (no external caller)
- `rate_limit: waived: "internal"` or `unlimited` (not user-rate-controlled)
- `observability: silent` or a single named event
- `availability: maintenance_window` or `best_effort`

v0.10 introduces `internal_action`, which auto-fills these four
slots with the canonical internal values:

```
surface {
  internal_action complete_invalidation(inv: InvId)
    by      sys: Admin
    when    acked[inv] == EDGE_IDS
    -- auth_channel: trusted_caller    (auto)
    -- rate_limit: waived: "internal"  (auto)
    -- availability: maintenance_window (auto, can be overridden)
    idempotency   : idempotent by(inv)        -- still required
    retention     : audit(period=YEAR)         -- still required
    observability : broadcast(InvalidationCompleted)   -- still required
    freshness     : not_applicable             -- still required
    then    pending := pending diff {inv}
            emit InvalidationCompleted(inv=inv)
}
```

Rules:

1. `internal_action` auto-fills `auth_channel: trusted_caller`,
   `rate_limit: waived: "internal"`, `availability:
   maintenance_window`. Any of these may be explicitly overridden by
   writing the slot.
2. The other four slots (`idempotency`, `retention`, `observability`,
   `freshness`) are **still mandatory** — internal actions still
   need design decisions about replay, retention, observation, and
   consistency.
3. `internal_action` requires `by sys: <ActorClass>` where the
   `<ActorClass>` is a designated internal actor (typically `Admin`
   or a module-defined `system`-class). External actors are
   rejected.
4. The docs projection labels internal actions distinctly so
   reviewers do not confuse them with user-facing actions.

#### 6.4.6.1 Slot resolution precedence (v0.10.1)

When `defaults { … }` (§6.4.5), `internal_action` presets (§6.4.6),
and per-action slot lines all apply to the same slot, the
**canonical resolution order** is:

```
internal_action preset  <  defaults  <  per-action slot line
       (lowest)                          (highest priority)
```

Read top-to-bottom: an `internal_action` preset is the **lowest
priority**, overridden by `defaults`, which is overridden by an
explicit per-action slot line. The reasoning:

- An `internal_action` keyword expresses "this is an internal
  action; if the surface specifies nothing further, use canonical
  internal presets." It is a *fallback*, not a force.
- A `defaults { … }` block expresses a *project-wide convention*,
  which intentionally overrides the keyword fallback.
- A per-action slot line expresses an *explicit design decision for
  this action*, which overrides everything else.

This means: if a `defaults { availability: critical }` is in scope,
an `internal_action` does **not** silently override to
`maintenance_window`; it inherits the project default. To get the
internal preset on a specific action, write `availability:
maintenance_window` explicitly.

The docs projection renders the **effective** slot value plus a
provenance tag: `(internal-preset)`, `(default)`, or no tag for
explicit. Reviewers can audit how each slot got its value.

> **v0.10.1 — provenance.** R8 (Opus, specific finding) flagged
> that v0.10 left the precedence implicit. The reading above is
> the only one consistent with the design intent of each
> construct: presets fall back, defaults express project intent,
> explicit lines win. The compiler errors with
> `E_SLOT_PRECEDENCE_AMBIGUOUS` if any implementation diverges.

This addresses the R7 unanimous finding that internal-action slot
ceremony was high-noise: four boring lines per internal action are
now one keyword.

### 6.5 State-field retention annotations (v0.10)

The §6.4.1 `retention: secret` static check on emit/log flows is
only realisable if individual state fields carry their retention
class. v0.10 lets `state { … }` declarations carry an inline
`retention:` annotation:

```
surface {
  state {
    -- Field-level retention. Carries the class through the type
    -- system; emit/log of a `retention: secret` field is a static
    -- error.
    signing_keys     : Map[KeyId -> Key]   retention: secret
    user_emails      : Map[UserId -> String] retention: pii(class=email, ttl=YEAR)
    cache_behavior   : Map[DistId -> CacheBehavior]    -- no annotation: ephemeral default
    audit_log        : Seq[AuditEntry]      retention: audit(period=DECADE)

    -- v0.10: `private` modifier — read by actor-relative observables only;
    -- direct reads from action bodies are warnings (style, not error).
    follows          : Map[(User, User) -> Bool] private
  }
}
```

Closed enum of state-field retention values: same as the action-slot
`retention` enum (§6.4.1), except `waived` is not allowed at the
state level (a state field has a definite class).

Cross-checks:

1. An `emit E(field=<expr>)` where `<expr>` resolves to a value of a
   `retention: secret` state field → static error
   `E_SECRET_FLOW(<event>, <field>, <source_field>)`.
2. An action that reads a `retention: pii(...)` field must declare
   `retention: pii(class>=<source_class>, ttl<=<source_ttl>)` or a
   stricter class. (Type-system-level monotonicity check.) Violations
   emit obligation `R-INFO-FLOW` (§15.4) which is acknowledgeable.
3. State fields are `ephemeral` by default (the same default as the
   action slot's loosest non-waived value).

The `private` modifier is **separate from retention** — it constrains
read-site convention, not data class. A field may be both `private`
and `retention: secret`; the two checks are independent.

### 6.6 `derived` surface state (v0.10; refined v0.10 per PL review)

A surface state field that is conceptually an aggregation or
projection of substrate state has appeared in three consecutive
agent rounds (R5, R6, R7) as the **longest-standing structural
friction in Surface**. v0.10 fixes it without violating the
surface/substrate abstraction boundary.

**The abstraction-preserving form (canonical).** A surface state
field is declared `derived`; the projection itself lives in each
substrate's `maps` block. The surface contract is *that the field
exists and is derived* — the surface does not name substrate
components.

```
surface {
  state {
    next_seq : Map[User -> Nat]      -- ordinary surface state
    posts    : Set[Post]  derived    -- "this field is derived; substrates
                                      -- must provide a projection."
  }

  action post(content: Content) -> PostId
    by u: User
    …
    then
      let id := (u, next_seq[u])
      next_seq[u] += 1
      -- No assignment to `posts`. It is derived; the substrate's
      -- projection materialises the new post after the substrate's
      -- own per-user action fires.
      emit Posted(id=id, author=u, content=content)
      return id
}

substrate PostStore realizes surface {
  replicate UserAccount[u: User in USERS] {
    state { my_posts : Set[Post] }
    …
  }

  -- The projection lives here. Every substrate realising this surface
  -- must define `posts` in `maps`:
  maps {
    posts = aggregate UserAccount[u].my_posts using union_set else {}
  }
}
```

Rules:

1. A `derived` field is **read-only on the surface side**. Any
   surface action body that assigns to it (with `:=`, `+=`, `:+`,
   `union`, …) is a static error: `E_DERIVED_ASSIGN(<field>)`.
2. Each substrate realising this surface **must** include the
   derived field in its `maps { … }` block. Omission is
   `E_DERIVED_NO_PROJECTION(<substrate>, <field>)`.
3. `init { posts := … }` is **forbidden** for derived fields — their
   initial value is determined by the substrate's initial state via
   the projection.
4. Properties and observables may read derived fields freely; events
   that include a derived field's value continue to work.
5. The `R-DERIVED-WRITE` obligation rule (§15.4) statically catches
   the case where an action body would assign — it is the same
   check as rule 1, surfaced as a friendly error rather than a
   refinement counter-example.
6. **Different substrates may project differently.** Substrate A
   may project `posts` from `UserAccount[u].my_posts`; substrate B
   may project it from `Shard[s].rows`. Both are valid as long as
   their projections produce values of the declared type. This is
   the property the v0.10 design protects.

**Why the boundary matters**: an earlier v0.10 draft allowed the
projection expression to live in the surface (`= derived from
<expr>`). That form named substrate components from the surface,
violating the v0.7 §7.1 separation. The PL review (Cut-One finding,
v0.10) flagged this as the least defensible v0.10 decision; the
final v0.10 form keeps the projection where it semantically
belongs — in the substrate's `maps` block.

**The `derived shape:` modifier (optional, v0.10).** For
documentation clarity, the surface may declare a *shape constraint*
without naming substrate components:

```
state {
  posts : Set[Post]  derived
    shape: per_actor                 -- one of: per_actor | aggregate | snapshot | indexed
    of: User                          -- only meaningful for per_actor / indexed
}
```

`shape:` is an annotation; it does not bind to substrate names. It
helps reviewers and the docs projection understand the *kind* of
projection without prescribing the architecture. The compiler does
not enforce shape (a `per_actor` field could be implemented as a
flat aggregate; the shape is a hint to readers).

**Why this matters in practice**: the v0.7 spec required authors to
mirror substrate aggregations as surface state and *re-assert* them
in action bodies (`edge_has[(d,p)] := true` after an Edge fetched).
The mirroring drifted: surface assignments did not always match
substrate behaviour, leading to refinement failures whose
counter-examples were hard to read. `derived` eliminates the mirror
— there is one source of truth (the substrate's actual state, via
its `maps` projection), and the surface field is a *named view* of
it.

Example with multi-substrate composition:

```
surface {
  state {
    edge_has : Map[(DistId, Path) -> Bool]  derived  shape: aggregate
  }
}

partial substrate EdgeMesh realizes surface owns { edge_has } {
  maps {
    edge_has = aggregate Edge[id].cache_valid using exists
  }
}
```

The `owns { edge_has }` partition is unchanged: the partial substrate
that owns the derived field is the one that must provide its
projection. Compose-level `cross_visible` aux state may participate
in projections; see §7.7.3 for the interaction.


## 7. The `substrate` block

### 7.1 What "refinement" is, and what auxiliaries are for

The surface defines the only behaviors a user can observe. The substrate
has its own, richer state and steps. **Refinement** is the claim: every
behavior the substrate can exhibit, when projected through the mapping, is
allowed by the surface. The compiler asks the model checker the contrapositive
— "find a substrate execution that projects to a surface state the surface
cannot reach" — and reports the trace as a counter‑example.

Two pieces are needed:

1. A **state mapping** — a function from substrate state (possibly augmented
   with auxiliary variables) to surface state.
2. An **action mapping** — for each substrate step, which surface action it
   realizes (possibly under a guard), or that it is internal (must leave the
   projected surface state unchanged).

**Auxiliary variables** (Abadi & Lamport, 1991) make the function in (1)
existable when it otherwise wouldn't be:

- `history h := …` — records past substrate facts so the projection can
  refer to them. Updated by ordinary substrate actions; never read by
  substrate code.
- `prophecy p := …` — guesses future nondeterministic choices the surface
  commits to *earlier* than the substrate does (e.g. saga outcome). Drawn
  nondeterministically; constrained by the action mapping to be consistent
  with the eventual outcome.

Auxiliaries are erased before TLC executes the *substrate* spec — they
exist only to make the refinement obligation provable.

### 7.2 Syntax

```
substrate <Name> realizes <SurfaceName> {
  component <CName> {
    state { … }
    init  { … }
    action <name>(<args>) when <Pre> then <Effect>
    receives <Msg>(<args>) from <Other>      -- handler form, see 7.2.1
    sends    <Msg>(<args>) to   <Other>      -- effect form, see 7.2.1
  }

  -- Value-parametric component: one instance per element of IDS.
  replicate <CName>[<id>: <IdType> in <IdsExpr>] {
    state { … }                                -- per-instance state
    action <name>(args) when <Pre> then <Effect>
    -- inside the component body, `id` refers to this instance's id.
  }

  channel <ChName> { from <C1> to <C2> }     -- v0.4: plain FIFO queue

  fairness weak <Component>.<action>
  fairness strong <Component>.<action>
  fairness weak <Component>[*].<action>      -- every replicated instance
  fairness weak <Component>[<id>].<action>   -- one specific instance
  fairness weak <surface_action>             -- shorthand: at least one
                                             -- realizing substrate action
                                             -- must have weak fairness;
                                             -- compiler picks one and
                                             -- requires it explicitly.

  auxiliary {
    history  h_log : Seq[Event]   := <>
    prophecy p_outcome : Outcome  := *
  }

  authentication {
    surface_actor of surface.<action> = <Component>.<field>[.<sub>…]
    surface_actor of surface.<action> = <Component>[<id>].<field>
    surface_actor of surface.<action> = system     -- system-internal actor
  }

  maps {
    <surface_field> = <expr over substrate ∪ auxiliary>
    ...
  }

  realizes {
    surface.<action>(args) by <Component>.<action>
      when <substrate_or_auxiliary_predicate>

    -- Surface action realised elsewhere (sibling substrate, manual op, etc.).
    -- Satisfies completeness without producing a refinement obligation here.
    surface.<action>(args) by EXTERNAL
  }

  internal {
    <Component>.<action>
    <Component>.*               -- glob: all actions of a component
    <Component>[*].<action>     -- glob: this action of every replicated instance
    <Component>[*].*            -- glob: everything in every replicated instance
  }
}
```

#### 7.2.1 Channels and `sends` / `receives`

A `channel` is a plain in‑order FIFO between two components. Either side
sends and receives via the corresponding effect/handler form:

```
component Saga {
  state { ... }
  action start(req) when … then
    sends DoDebit(req=req) to AccountSvc
    pending[req.id] := req
}

component AccountSvc {
  state { table : Map[AccountId -> Nat] }
  receives DoDebit(req: TxReq) from Saga
    when table[req.from] >= req.amt
    then table[req.from] -= req.amt
         sends DebitOk(req_id=req.id) to Saga
}
```

A `receives` declaration is itself an action of the receiving component
(it appears in `realizes`/`internal` like any other). `sends` inside an
action body appends to the channel; the message is delivered as a separate
`receives` step on the other end. To model an unreliable channel, declare
an explicit `internal` action that drops or duplicates messages.

When either endpoint is a replicated component, the `from`/`to` clause
must say what the multiplicity means:

```
channel C { from A             to B          }   -- 1:1
channel C { from A             to B[*]       }   -- broadcast: each `sends`
                                                 -- delivers to every B[id]
channel C { from A[*]          to B          }   -- fan-in: any A[id] sends,
                                                 -- B receives one copy
channel C { from A[i]          to B[i]       }   -- pairwise: A[id] talks to
                                                 -- B[id] (same id space)
channel C { from A[*]          to B[*]       }   -- N×M; sends require an
                                                 -- explicit `to B[<expr>]`
                                                 -- destination at the call site
```

The compiler rejects ambiguous combinations. The `pairwise` form requires
both endpoints to be replicated over the same id set.

The call‑site forms of `sends`:

```
sends Msg(args)                                  -- shorthand: when there is
                                                 -- exactly one channel between
                                                 -- this component and any
                                                 -- destination, the channel is
                                                 -- inferred.
sends Msg(args) to <ChannelName>                 -- canonical: name the channel
sends Msg(args) to <Component>                   -- shorthand for "the unique
                                                 -- channel from us to this
                                                 -- component."
sends Msg(args) to <Component>[<idExpr>]         -- one specific replicated
                                                 -- instance, when the channel
                                                 -- type is N×M.
```

For **cross‑substrate channels declared in `compose`** (§7.7.2), use the
channel name. The name is in scope for any component of either composed
substrate; do not qualify it with the substrate or the remote component:

```
-- Inside ControlPlane.InvalidationQueue, sending across to EdgeMesh's edges:
sends Purge(d=d, paths=paths) to PurgeFanout

-- Inside EdgeMesh.Edge[id], acking across to ControlPlane:
sends PurgeAck(id=id, inv=inv) to PurgeAckFanin
```

The compiler checks that the channel is in scope (declared in a
`compose` that includes this substrate) and that the message type
matches the channel's declared type.

#### 7.2.2 `replicate` worked example

```
extern EDGE_IDS : Set[EdgeId]

substrate EdgeMesh realizes CDN.surface {

  replicate Edge[id: EdgeId in EDGE_IDS] {
    state { cache: Map[Path -> Body]; cache_valid: Map[Path -> Bool] }
    init  { cache := {}; cache_valid := { p -> false | p in PATHS } }
    action serve(p: Path) when cache_valid[p] then
      sends Served(edge=id, p=p, body=cache[p]) to ResultBus
  }

  fairness weak Edge[*].serve

  realizes {
    surface.get(d, p) by Edge[id].serve
      for some id when ...
  }
}
```

Each replicated instance has its own state (`cache`, `cache_valid`) and
its own actions, indexed by `id`. `Edge[*]` in `fairness` and `internal`
quantifies over every instance.

#### 7.2.3 Aggregating replicated state in `maps` (v0.6, refined v0.7)

A `maps { … }` clause that needs to project over many replicated
instances would otherwise have to be written by hand with `choose`
and `exists`:

```
edge_body = { (d, p) -> Edge[choose id: EdgeId. Edge[id].cache_valid[(d,p)]].cache[(d,p)]
              | (d, p) in DISTS cross PATHS,
                exists id in EDGE_IDS. Edge[id].cache_valid[(d,p)] }
```

That pattern is the most common substrate‑to‑surface mapping in any
sharded model and the v0.5 evaluation flagged it as the worst part of
real specs. v0.6 introduced the `aggregate` form; v0.7 tightens the
typing rules so the corner cases stop biting:

```
maps {
  edge_valid = aggregate Edge[id].cache_valid using exists
  edge_body  = aggregate Edge[id].cache
                  over   { id in EDGE_IDS | Edge[id].cache_valid }
                  using  union_set
                  else   {}
}
```

Grammar:

```
aggregate <Component>[<id>].<expr>
  [ over <ScopeExpr> ]                    -- subset of ids; defaults to *
  using <Aggregator>
  [ else <FallbackExpr> ]                 -- value when scope is empty;
                                          -- required for max/min and any
                                          -- other aggregator without a
                                          -- canonical empty value
```

Built‑in aggregators (v0.7):

| Aggregator         | Type rule                       | Empty scope behavior          | Meaning                                                  |
|--------------------|---------------------------------|-------------------------------|----------------------------------------------------------|
| `exists`           | `Bool ← Set[Bool]`              | `false`                       | logical OR over the scope                                |
| `forall`           | `Bool ← Set[Bool]`              | `true`                        | logical AND over the scope                               |
| `sum`              | `Nat \| Int ← Set[Nat \| Int]`  | `0`                           | numeric sum                                              |
| `max` / `min`      | `T ← Set[T]` (comparable)       | requires `else <expr>`        | extreme element                                          |
| `union_set`        | `Set[T] ← Set[Set[T]]`          | `{}`                          | flatten and union (set semantics; deduplicating)         |
| `concat_seq(order_by f)` | `Seq[T] ← Set[Seq[T]]`    | `<>`                          | flatten‑and‑concatenate; `f` orders the contributing ids |

> **v0.7 cuts:** `using all` and `using any` (v0.6) are **removed**.
> `all` was just a comprehension `{ Component[id].expr | id in scope }`;
> `any` was just `choose x in { … }. true`. Cutting them eliminates the
> non-determinism warning that came with `any` and shrinks the
> aggregator matrix.

> **v0.7:** the v0.6 `using union` keyword is renamed to `union_set`.
> The previous `union` was ambiguous over `Seq` (which is type-incoherent
> as a flatten); `union_set` is set-semantics only. `concat_seq` is the
> new sequence form; it requires an `order_by` to make the concatenation
> deterministic.

Examples (v0.7):

```
-- "true if any replica has it"
edge_valid[(d, p)] = aggregate Edge[id].cache_valid[(d, p)] using exists

-- The most recent body any valid replica knows. Now uses union_set on
-- the singleton-set wrapping pattern, then `choose` to extract one.
edge_body[(d, p)] = (let candidates :=
                       aggregate { Edge[id].cache[(d, p)] }
                         over { id | Edge[id].cache_valid[(d, p)] }
                         using union_set
                         else  {}
                     in choose b in candidates. true)

-- A per-replica counter sum
total_serves = aggregate Edge[id].serve_count using sum

-- A set built by set-union over per-replica sets
known_paths(d) = aggregate Edge[id].known[d] using union_set

-- v0.7 worked example: per-replica MAP aggregating to a surface MAP.
-- Strategy: a plain map comprehension over the cross product of
-- replica ids and per-replica keys. No `aggregate` is needed (and
-- `union_set` would be type-incoherent on a map). Use the
-- comprehension form when the result is a map keyed by `(id, k)`.
per_user_state = { (id, k) -> Edge[id].local_state[k]
                   | id in EDGE_IDS, k in Edge[id].local_state.keys }

-- For comparison, a Set-valued aggregate (where union_set is the
-- right operator) looks like:
-- known_paths_per_replica = aggregate Edge[id].known
--                             using union_set
```

`aggregate` is itself an expression and may appear anywhere a value
expression is allowed: `maps`, `observable`, `history_predicate`, and
(v0.7) inside action `then` blocks too (§6.1).

#### 7.2.4 Named epochs (v0.10)

A substrate may declare one or more **named epochs** to give the
surface's `freshness:` slot (§6.4.1) a precise meaning. An epoch is
a *symbolic clock* that advances when a designated set of substrate
actions fires, over a designated set of state fields.

Grammar:

```
epoch <name> {
  advances_on  <action-or-receives-pattern> [ , <pattern> ]*
  covers       <field> [ , <field> ]*
}
```

Example (EdgeMesh substrate):

```
substrate EdgeMesh realizes CDN.surface {
  replicate Edge[id: EdgeId in EDGE_IDS] {
    state { cache_valid : Map[Path -> Bool] }
    receives Purge(inv: InvId, paths: Set[Path]) from PurgeFanout
      then for p in paths do cache_valid[p] := false
    …
  }

  -- A "purge_propagation" epoch advances once per fanout receive
  -- across every Edge[*]. It covers the cache_valid state.
  epoch purge_propagation {
    advances_on  Edge[*].receives.Purge
    covers       cache_valid
  }

  -- A "publish_propagation" epoch advances when the OriginShield
  -- pushes a published body out.
  epoch publish_propagation {
    advances_on  OriginShield.broadcast_publish
    covers       cached_body
  }
}
```

The surface action's freshness slot may now reference these:

```
action get(d, p) -> Body by v: Viewer
  …
  freshness: bounded(purge_propagation, n=1)
  -- "after a purge fires, every Edge has applied it within one
  --  purge_propagation epoch step (i.e. one receive per replica)."
  then …
```

Rules:

1. Every epoch name in a freshness slot must be declared by **at
   least one** realising substrate; the compiler errors with
   `E_FRESHNESS_UNDECLARED_EPOCH` otherwise.
2. Different substrates may declare epochs of the same name with
   different `advances_on` sets — the freshness obligation is checked
   per substrate, against that substrate's definition.
3. The compose may declare a cross-substrate epoch:
   `epoch <name> { advances_on <Sub1.action>, <Sub2.action> covers
   <surface_field> }`.
4. An epoch's `covers` field set must overlap with the state fields
   the freshness-slotted action reads (compiler-checked); otherwise
   the obligation is vacuous and the freshness slot is misleading.
5. Liveness obligations: `eventual(<epoch>)` requires `fairness weak
   <action>` on every action listed in the epoch's `advances_on`, or
   the freshness becomes unsatisfiable. The compiler warns if the
   fairness annotation is missing.

This gives `freshness: bounded(...)` a precise model: a finite
counter generated from the named epoch's advance events; the
refinement obligation is that no write older than `k` epochs is
invisible to the slotted action.

> **PL-review provenance.** Originally `bounded(epochs=n)` named no
> epoch and was deemed "neither rigorous nor honest" by the PL
> review. v0.10 ships the named-epoch form before R8.

### 7.3 Completeness rule (and `EXTERNAL`)

Every concrete substrate action **must** appear in either `realizes` or
`internal`, and not both (default‑deny within a substrate). The compiler
emits an error listing any action appearing in neither.

Additionally, **every surface action must be realized by a non‑`EXTERNAL`
clause across the union of all substrates that realize the same surface**.
The compiler emits a project‑level error if a surface action is `EXTERNAL`
in every substrate (or has no `realizes` clause anywhere). This catches
the "nobody actually implements this" mistake when splitting data plane
and control plane across substrates.

Use `by EXTERNAL` for surface actions that this substrate genuinely does
not realize — typically control‑plane actions when the substrate models
only the data plane, or actions performed by an out‑of‑band operator. A
`by EXTERNAL` clause satisfies completeness here and produces no
refinement obligation.

### 7.4 Internal actions and stuttering

An action listed in `internal` generates the proof obligation
`Action ⇒ UNCHANGED Map(surface_vars)`. If the obligation fails, the user
sees a counter‑example trace highlighting the offending step rather than a
generic refinement failure.

A substrate that internally loops forever without taking any `realizes`
step is *divergent*. Divergence is fine for surface safety but breaks every
surface liveness claim. The compiler warns when a substrate has an
`internal`‑only cycle reachable from `Init`.

#### 7.4.1 `Component[*].receives.MsgName` form (v0.7)

Receives handlers are component actions per §7.2.1 and may be listed in
`realizes`/`internal`/`fairness` like any other action. The path
syntax is:

```
internal { Edge[*].receives.Purge }                -- one handler, every replica
internal { ControlPlane.Index.receives.Subscribe } -- one handler, one component
fairness weak Edge[*].receives.OriginResponse
```

The `.receives.MsgName` segment names the handler; the compiler
generates a synthetic action name `receives_<MsgName>` internally.

### 7.5 Authentication mapping

For attacker reasoning to be sound, every realized surface action must show
how the actor identity required by the action's `by <Actor>` parameter is
determined from substrate state. The `authentication { … }` block names
the substrate location that carries the identity.

Grammar of the right‑hand side:

```
<RHS> ::= <Component>.<field>[.<sub>…]              -- a path into a component's state
       |  <Component>[<id>].<field>[.<sub>…]         -- a path into a replicated instance
       |  param.<argname>                            -- v0.5: an action parameter,
                                                     -- e.g. the `v: Viewer` of
                                                     -- `action get(...) by v: Viewer`
       |  system                                     -- the action has no caller; the
                                                     -- "actor" is the system itself
                                                     -- (use for control-plane housekeeping
                                                     -- actions like cache eviction)
```

`param.<name>` resolves to the action's `by` parameter (or any other
named parameter of an actor type) at the moment of realization.
Recommended for message‑passing systems where the actor identity rides
on the request, not in long‑lived component state.

A surface action realised `by EXTERNAL` does not require an
`authentication` entry in this substrate (the realising substrate must
provide one). Without an entry for a non‑external realising clause, the
compiler refuses to enable attacker checks.

### 7.5.1 Realization parameter binding scope (v0.7)

The names in scope inside a `realizes` `when` guard are:

| Origin                        | Form                                          | Example                                          |
|-------------------------------|-----------------------------------------------|--------------------------------------------------|
| Surface action arguments      | bare name                                      | `pid`, `target`, `content`                       |
| The actor parameter            | `param.<argname>`                              | `param.u`, `param.v`                              |
| Substrate action arguments    | `<Component>[<id>?].<action>.<arg>`            | `Edge[id].serve.p`, `Db.commit.txn`              |
| Component state               | `<Component>[<id>?].<field>[.<sub>…]`          | `Edge[id].cache_valid[(d, p)]`                    |
| Auxiliary variables           | `<Component>?.<aux>` (component-qualified or substrate-level) | `acked[inv]`                          |
| Cross-substrate state         | `<OtherSubstrate>.<…>` (only inside compose)   | `FollowGraph.Index.edges[(u, t)]`                 |

The `<id>` of a replicated component is bound **existentially by
default**: a bare `Component[id]` reference in a `realizes` clause
introduces a fresh existential `id` ranging over the replicate's id
set, scoped to that clause. To pin the binder to a specific value,
write `Component[<expr>]`. To make the existential explicit (or to
share it across multiple clauses), prefix with `for some <id> in
<IdsExpr>`:

```
-- Implicit existential (the common case):
realizes {
  surface.post(content) by UserAccount[u].publish
    when UserAccount[u].publish.actor_user == param.u
      && UserAccount[u].publish.content    == content
  -- `u` here is bound existentially over USERS; the constraint
  -- `UserAccount[u].publish.actor_user == param.u` pins it to the
  -- caller's identity.
}

-- Explicit form when you want the binder named outside the clause:
realizes {
  for some u in USERS.
    surface.post(content) by UserAccount[u].publish
      when UserAccount[u].publish.content == content
}
```

The replicate binder is **not** an action parameter and must not
appear after `param.`.

A `realizes when …` guard may freely mix these names; the compiler
binds them existentially over the substrate action's variables (so
`Edge[id].serve.p == p` constrains `id`, `Edge[id].serve.p`, and the
surface argument `p` to be consistent).

### 7.6 Refinement obligation, formally

For substrate `S` realizing surface `Σ`, the compiler emits:

```
S_with_aux ∧ Fairness_S  ⇒  Map(Σ ∧ Fairness_Σ)
```

Where `S_with_aux` is the substrate spec extended with the auxiliary
variable updates, and `Map` is the substitution defined by `maps`. The
realization clauses determine which substrate next‑state disjunct is
mapped to which surface next‑state disjunct.

### 7.7 Partial substrates and `compose` (v0.5)

Many real systems are built from peer subsystems that *together* implement
one user‑visible surface — most commonly a data plane and a control plane.
v0.4 supported splitting *actions* across substrates with `by EXTERNAL`,
but every substrate still had to map every surface state field, which
forced dummy mappings for fields the substrate did not own. v0.5 adds
**field ownership** so peer substrates split state too.

#### 7.7.1 `partial substrate … owns { … }`

```
partial substrate EdgeMesh realizes CDN.surface
  owns { cache_valid, cached_body, viewer_geo }
{
  -- only the owned fields appear in `maps`:
  maps {
    cache_valid = ...
    cached_body = ...
    viewer_geo  = ...
  }
  realizes { surface.get(d, p) by Edge[id].serve_hit when ... }
  -- create_invalidation, publish, etc. are `by EXTERNAL` here as before.
}

partial substrate ControlPlane realizes CDN.surface
  owns { pending_invalidations, cache_behavior, origin_body }
{
  maps {
    pending_invalidations = ...
    cache_behavior        = ...
    origin_body           = ...
  }
  realizes { surface.create_invalidation(...) by ...
             surface.update_cache_behavior(...) by ...
             surface.publish(...) by ... }
}
```

A *plain* `substrate` (no `partial`) is shorthand for `partial substrate
… owns { <every surface field> }` — it owns and must map everything.

#### 7.7.2 `compose` — joining peer substrates

A `compose` block at the project level wires partial substrates into a
complete realization and declares any channels that cross substrate
boundaries:

```
compose Production = EdgeMesh + ControlPlane {

  -- channels that span substrates live here, not inside either substrate
  channel PurgeFanout {
    from ControlPlane.InvalidationQueue to EdgeMesh.Edge[*]
  }

  channel PublishFanout {
    from ControlPlane.OriginStore       to EdgeMesh.OriginShield
  }
}
```

The compiler enforces, project‑wide:

1. **Field‑ownership partition**: every surface state field is owned by
   exactly one substrate in the `compose`. Two substrates owning the same
   field is an error; an unowned field is an error.
2. **Action coverage**: every surface action is realized by at least one
   substrate in the `compose` non‑`EXTERNALly`.
3. **Channel completeness**: every `sends` to a cross‑substrate channel
   has a matching `receives` in the other substrate.

The composite refinement obligation is the conjunction of each partial
substrate's obligation, with cross‑substrate channels treated as shared
state. Composition is associative; `A + B + C` is well‑defined.

A `compose` block may also restate fairness (e.g.
`fairness weak ControlPlane.InvalidationQueue.dequeue`) when the relevant
fairness only makes sense at the joined level.

#### 7.7.3 Cross‑substrate `realizes` overrides (v0.6)

A `realizes` clause inside a partial substrate may only reference its
own substrate's state. But many joint surface actions need a guard that
spans both substrates — e.g. *"`complete_invalidation` fires only when
every edge has acknowledged."* The `realizes { … }` block inside a
`compose` may add or strengthen such guards. Inside a compose `realizes`,
both substrates' state (including their `auxiliary { … }` variables)
are in scope by qualified name:

```
compose Production = EdgeMesh + ControlPlane {

  channel PurgeFanout    { from ControlPlane.InvalidationQueue to EdgeMesh.Edge[*] }
  channel PurgeAckFanin  { from EdgeMesh.Edge[*] to ControlPlane.InvalidationQueue }

  realizes {
    -- Strengthen the existing realizing clause: the substrate-local
    -- realizing action stays the same, but the compose adds a precondition
    -- that depends on the *other* substrate's auxiliary state.
    surface.complete_invalidation(inv) by ControlPlane.InvalidationQueue.complete
      when EdgeMesh.acked[inv] == EDGE_IDS    -- cross-substrate guard
  }
}
```

Rules:

1. A compose‑level `realizes` clause may only **strengthen** (logical
   `AND` with) an existing substrate‑level `realizes` of the same
   surface action; it cannot weaken or replace one. The compiler checks
   this.
2. The cross‑substrate guard is added to the project‑level refinement
   obligation. It does *not* change either partial substrate's
   independent obligation (each remains independently checkable for its
   own fields).
3. To expose an internal/auxiliary variable for cross‑substrate
   reading, the owning substrate must declare it with the
   `cross_visible` modifier:

   ```
   auxiliary {
     history acked : Map[InvId -> Set[EdgeId]] := {} cross_visible
   }
   ```

   `cross_visible` adds the variable to the substrate's "exported"
   surface for compose; without it, the compose cannot read it. This
   keeps each substrate's encapsulation explicit.

This addresses the v0.5 evaluation finding that joint surface steps
(every‑edge‑has‑acked, quorum reached, distributed commit) had no
clean spec home.

##### `cross_visible` is a two‑way contract (v0.7)

The owning substrate of a `cross_visible` aux variable declares its
type and initial value; the **invariant** is also declared at that
site. Either composed substrate may read it (in compose `realizes`
guards, fairness annotations, etc.) **and** either may *write* it via
explicit assignment in their own action bodies. The compose-level
refinement obligation requires the invariant to hold across all such
writes.

Concretely:

```
-- Owning substrate (EdgeMesh): declare with invariant.
auxiliary {
  history acked : Map[InvId -> Set[EdgeId]] := {} cross_visible
    invariant forall inv. acked[inv] subset EDGE_IDS
}

-- Either substrate may write it. EdgeMesh's Edge[id] writes when the
-- purge handler fires:
component Edge[id] {
  receives Purge(inv: InvId) from PurgeFanout
    then acked[inv] := acked[inv] union {id}
}

-- ControlPlane.InvalidationQueue may also read it (via compose
-- realizes) AND may itself write it, e.g. on rollback:
component InvalidationQueue {
  action rollback(inv: InvId) when … then
    acked[inv] := {}                 -- legal because acked is cross_visible
}
```

The compiler enforces that every write to a `cross_visible` variable
preserves the declared invariant. If the variable's owning substrate
disappears under refactoring, the compiler errors at every cross-write
site.

#### 7.7.4 Cross‑substrate channels: a worked example (v0.7)

Cross‑substrate channels declared in `compose` follow the §7.2.1
shape. Worked end‑to‑end example: control plane sends purges, data
plane acks back.

```
-- compose.surf
compose Production = EdgeMesh + ControlPlane {
  channel PurgeFanout    { from ControlPlane.InvalidationQueue to EdgeMesh.Edge[*] }
  channel PurgeAckFanin  { from EdgeMesh.Edge[*]               to ControlPlane.InvalidationQueue }
}

-- EdgeMesh.Edge[id] (in data_plane.surf): handles incoming purges,
-- updates the cross_visible aux, and acks back through the fan-in
-- channel.
component Edge[id] {
  state { cache_valid : Map[Path -> Bool] }

  receives Purge(inv: InvId, paths: Set[Path]) from PurgeFanout
    then for p in paths do cache_valid[p] := false
         acked[inv] := acked[inv] union {id}            -- cross_visible aux
         sends PurgeAck(inv=inv, edge=id) to PurgeAckFanin
}

-- ControlPlane.InvalidationQueue (in control_plane.surf): receives
-- the acks and can complete the invalidation when every edge is in.
component InvalidationQueue {
  state { pending : Set[InvId] }

  action begin(inv: InvId, paths: Set[Path]) then
    pending := pending union {inv}
    sends Purge(inv=inv, paths=paths) to PurgeFanout

  receives PurgeAck(inv: InvId, edge: EdgeId) from PurgeAckFanin
    then  -- ack accounting handled by `acked` aux on the EdgeMesh side;
          -- the receive itself just keeps the channel drained.

  action complete(inv: InvId)
    when inv in pending && acked[inv] == EDGE_IDS
    then pending := pending diff {inv}
         emit InvalidationCompleted(inv=inv)
}
```

Both the `sends` and the `receives` reference the channel by name
(`PurgeFanout`, `PurgeAckFanin`). The `internal` block in each
substrate lists the receives handler with the
`Component[*].receives.MsgName` form (§7.4.1).

#### 7.7.5 `by stutter` for no‑effect branches (v0.7)

A surface `if/else` branch that is observably a no‑op (e.g. the
`[redundant]` branch of an idempotent `follow`) needs *some* substrate
realization to satisfy the branch coverage rule (§6.1.2). Rather than
forcing authors to add an explicit no‑op action to a substrate
component, v0.7 introduces a built‑in compose‑level realization
target:

```
compose Production = PostStore + FollowGraph {

  realizes {
    surface.follow(target)[redundant]      by stutter
    surface.unfollow(target)[noop]         by stutter
    surface.set_protected(p)[noop]         by stutter
  }
}
```

`by stutter` compiles to the obligation `UNCHANGED Map(<surface_vars>)`
for that branch. It is legal **only at compose level** (not inside a
single partial substrate), because the obligation is project‑wide:
both substrates must be unchanged on this transition.

> **v0.7 cut:** the informal `by no_op` pseudo-action used in earlier
> drafts is **rejected** by the compiler. Use `by stutter` instead.

#### 7.7.6 Backwards compatibility

A v0.4 spec with one full `substrate` continues to check unchanged. The
new constructs are opt‑in: write `partial substrate` and a `compose` only
when you actually have peers. Single‑substrate systems should not pay
the syntactic cost.

## 8. `scenario` — executable use cases

```
scenario "Alice pays Bob" tags: [happy_path, billing] {
  actors  { Alice: Customer, Bob: Customer }
  given   balances == { A: 100, B: 0 }
          owner    == { A: Alice, B: Bob }
  when    Alice does transfer(A, B, 30)
  then    balances == { A: 70, B: 30 }
          observed Transferred(from=A, to=B, amount=30) by Alice
}
```

Every scenario is **explicitly classified** as one of:

```
scenario "..." kind: safety { ... }      -- default; checked at every substrate
scenario "..." kind: liveness { ... }    -- requires opt-in substrate list
scenario "..." kind: forbidden { ... }   -- the trace must not be reachable
```

- `safety` scenarios are checked at the surface for invariant preservation
  along the trace; substrates inherit them by refinement.
- `liveness` scenarios require `requires_in: [SubstrateA, SubstrateB]` and
  are checked there with the relevant fairness annotations. Without the
  list, the compiler errors — different substrates *can* legitimately
  refuse to exhibit a happy‑path trace (e.g. one that deliberately rejects
  all writes), and silently failing them is wrong.
- `forbidden` scenarios assert the `when` sequence cannot reach the `then`
  state.

> **v0.7 clarification:** asserting that an action *deterministically
> fails with a named error* is a positive assertion about a **reachable**
> state of the failure path. The correct kind is `safety` with a
> `then fails with X` clause, **not** `forbidden`. `forbidden` is for
> traces that should not exist at all (security violations, illegal
> orderings).
>
> ```
> scenario "double-spend is rejected" kind: safety {        -- ✓ canonical
>   …
>   then fails with InsufficientFunds
> }
>
> scenario "no replay attack succeeds" kind: forbidden {     -- ✓ canonical
>   …
>   then balance(A) < 0                                       -- this state is unreachable
> }
> ```

Sequencing inside a `when` block is **interleaved by default**: each `does`
line is one surface action and other actors' steps may interleave. Use
`when atomic { … }` to require a contiguous sequence (compiles to a
sequential composition obligation).

## 9. `property` — invariants and temporal claims

```
property no_money_creation { always sum(balances.values) == initial_total }
property liveness_transfer { eventually committed_count > 0 }
```

`always P` is `[]P`. `eventually P` is `<>P`. There is no `invariant`
keyword; use `property name { always P }`. (This is the resolution to the
v0.2 invariant/property duplication.)

Properties may be referenced by name from substrates and scenarios.

### 9.1 `history_predicate` — reusable event‑log predicates (v0.5)

Properties about *what has happened* over the event log get unreadable
fast (`exists e in events. e is X && forall later in events_after(e). …`).
A `history_predicate` is a named, parameterized predicate over the event
log that can be reused inside any boolean position:

```
history_predicate after_invalidation(d: DistId, p: Path) {
  exists e in events. e is InvalidationCompleted
                   && e.d == d
                   && p in e.paths
}

history_predicate stale_served(d: DistId, p: Path) {
  exists e in events. e is Served
                   && e.d == d && e.p == p && e.cache_hit
                   && (exists pub in events_before(e).
                         pub is Published && pub.p == p
                         && origin_body[p] != e.body)
}

property no_stale_after_invalidation {
  always (forall d, p.
            after_invalidation(d, p) =>
              not stale_served_since(d, p, last(after_invalidation(d, p))))
}
```

### 9.1.1 The event log and its helpers (v0.6, refined v0.7)

The surface block has an implicit state variable
`events: Seq[Event]` (§5.1) — a chronologically ordered sequence, not a
set. The helpers used inside `property` and `history_predicate` bodies
have these signatures:

```
-- Slicing helpers
events_before(e: Event) : Seq[Event]              -- the prefix of `events` strictly before e
events_after(e: Event)  : Seq[Event]              -- the suffix strictly after e
between(a: Event, b: Event) : Seq[Event]          -- events strictly between a and b

-- Search helpers (v0.6 unbounded form, v0.7 bounded form added)
first(p: Event -> Bool)                  : Optional[Event]   -- earliest e in `events` satisfying p
first(seq: Seq[Event], p: Event -> Bool) : Optional[Event]   -- earliest in seq

last(p: Event -> Bool)                   : Optional[Event]
last(seq: Seq[Event], p: Event -> Bool)  : Optional[Event]

count(p: Event -> Bool)                  : Nat
count(seq: Seq[Event], p: Event -> Bool) : Nat
```

The bounded forms (v0.7) take a `Seq[Event]` first argument, almost
always `events_before(e)` or `events_after(e)`. They make
time-relative predicates expressible without Optional plumbing:

```
-- Was viewer following target by the time event e happened?
let f := count(events_before(e), x is Followed   && x.follower == viewer && x.target == target)
let u := count(events_before(e), x is Unfollowed && x.follower == viewer && x.target == target)
f > u
```

All helpers are pure and may appear in any boolean / value position.
The predicate argument is a *pattern* over a single event variable
named `e` by convention; you may rename it (`last(events_before(e2), x is X && x.f == y)`).

#### Duplicate-event semantics (v0.7)

Events are equal-by-value records, so two distinct emissions can
compare equal. The slicing helpers reason in terms of **occurrence
indices** in `events`, not values:

- `events_before(e)` uses the **first** occurrence of `e` in `events`.
- `events_after(e)`  uses the **last** occurrence.
- `between(a, b)` uses the first occurrence of `a` and the last of `b`.

When this matters, add a serial field to the event
(`event Foo(... seq: Nat)`) so each emission is value-distinct.

### 9.1.2 `state_at(e)` — surface state at a past event (v0.7)

History predicates that ask *"was condition C true when event e
fired?"* need a snapshot of state at the time `e` was emitted, not
current state. `state_at(e)` returns such a snapshot:

```
state_at(e: Event) : SurfaceState        -- a record of all surface state fields
                                          -- as they were when `e` was emitted
```

Field access works as on the live state:

```
history_predicate was_protected_at(target: User, e: Event) {
  state_at(e).protected[target]
}

property no_unauthorized_disclosure {
  always (forall e in events.
            e is Disclosed =>
              e.author == e.to
              || state_at(e).follows[(e.to, e.author)]
              || not state_at(e).protected[e.author])
}
```

`state_at` is a value expression; like every event-log helper it is
pure and may appear in any boolean / value position. The compiler
materialises the snapshot from the event-log replay; users do not
need to maintain an explicit history variable for this.

**Duplicate-event semantics.** Like `events_before`/`events_after`
(§9.1.1), `state_at(e)` reasons in terms of occurrence indices. When
the event value `e` appears multiple times in the log, `state_at(e)`
returns the snapshot **immediately after the first occurrence** of
`e` (consistent with `events_before(e)` using the first occurrence
to define its prefix). Authors who need to disambiguate should add a
serial field to the event so each emission is value-distinct.

`history_predicate`s themselves are pure: no effects, no side state.
They can reference surface state (via `events`, `state_at(e)`, and any
surface `state` field) but not substrate state. Recursion is rejected.

## 10. `attacker` — security as integrity reachability

**Scope (v0.3): integrity / authorization‑bypass only.** This is a
reachability check ("does there exist a trace where attacker‑controlled
actions cause an effect attributable to someone else?"). Confidentiality
and non‑interference are hyperproperties (a relation between *pairs* of
traces) and are not expressible as TLA+ reachability. The compiler rejects
attacker `goal` clauses that reference notions like "the attacker learns
X" — they must be reformulated as "the attacker causes effect Y."

```
attacker LowPrivReader {
  controls  u: User
  initial   role_of[(SECRET_DOC, u)] == Reader
  may       any action whose permission holds for u
  goal      eventually emits Wrote(d=SECRET_DOC, by=u)
}
```

The compiler:

1. Rewrites `goal` into a surface‑level `<>Goal` property.
2. Adds an attacker process that nondeterministically picks any enabled
   action of `u` per step.
3. Asks TLC for a counter‑example. A satisfying trace *is* the exploit
   chain.

Because attacker checks reason about actor identity, they require every
substrate they run against to declare an `authentication { … }` mapping
(§7.5).

#### 10.1 `extern` is top‑level only (v0.7)

The `extern <Name> : <Type>` declaration form (§2) is **module-level
only**. It cannot appear inside `attacker`, `scenario`, `surface`,
`substrate`, or `compose` blocks. Attacker blocks reference
module-level externs by their bare name:

```
extern TARGET : User                    -- top-level, in surface.surf

attacker NonFollowerSnoop {              -- references TARGET below
  controls   snoop : User
  initial    protected[TARGET] == true && not follows[(snoop, TARGET)]
  may        any action allowed for snoop
  goal       eventually emits Disclosed(... author=TARGET, to=snoop)
}
```

This restriction is intentional: externs are part of the model
configuration and need a global name so the `.cfg` can bind them.

## 11. Documentation extraction

Every declaration accepts a doc‑string immediately above:

```
"""
Transfer `amount` from `from` to `to`. Atomic: either both balances change
or neither does. Subject to ownership.
"""
action transfer(...) ...
```

`surface emit docs` produces, per module:

- One Markdown page per actor: actions they can take (with prose,
  preconditions translated to readable English, raised errors), events
  they can observe, worked scenarios.
- One page per substrate: components, channels, the `maps` projection
  rendered as a table.

A worked sample of the rendered Markdown ships at
`examples/url-shortener/docs-sample.md` (v0.7).

## 12. Tooling

Surface has its own toolchain. TLA+/PlusCal codegen is one **backend**
among several; the language's static analyses (slot checking, refinement
preconditions, the obligation pass) live in the Surface frontend and run
before any TLA+ is produced.

```
$ surface check       file.surf       # default pipeline: slots → obligations →
                                       # refinement → safety + scenarios
$ surface check --slots               # only §6.4 mandatory-slot pass
$ surface check --obligations         # only §15 static-obligation pass
$ surface check --refinement          # only the surface↔substrate refinement
                                       # obligation (requires TLA+ backend)
$ surface check --liveness            # also check liveness (needs fairness;
                                       # requires TLA+ backend)
$ surface check --attacker LowPriv    # integrity reachability (TLA+ backend)
$ surface emit tla     file.surf -o out/  # surface → TLA+ state machine
$ surface emit pluscal file.surf -o out/  # substrate → PlusCal
$ surface emit gherkin file.surf -o features/
$ surface emit docs    file.surf -o site/
$ surface migrate <from> <to>             # mechanical version migration
                                          # (currently: v0.7 → v0.9, §6.4.4)
```

**Tooling separation (v0.9 reframe).**

| Pass                 | Where it runs      | Catches                                  |
|----------------------|--------------------|------------------------------------------|
| `--slots`            | Surface frontend   | Missing/illegal mandatory coverage slots |
| `--obligations`      | Surface frontend   | Unacknowledged derived consequences (§15)|
| `--refinement`       | TLA+ backend       | Substrate diverges from surface          |
| `--liveness`         | TLA+ backend       | `eventually` fails under fairness        |
| `--attacker`         | TLA+ backend       | Integrity-reachability counter-example   |

The frontend passes are **deterministic and fast**: they require no model
checker and produce Rust-style local errors with a fix site. They run
in CI on every PR. The backend passes are exhaustive state-space
explorations: they produce counter-example traces and are typically
run on a schedule or before release.

> **v0.9 positioning.** Earlier drafts described Surface as "a language
> that compiles to TLA+." That understates the frontend. Surface is a
> language for **specifying systems by their boundaries**, with a
> static checker for boundary completeness and a TLA+/PlusCal
> backend for refinement and temporal model checking. Authors who
> never invoke `--refinement` still get genuine value from the
> frontend (the slot pass alone catches most "you forgot to specify
> idempotency" / "you didn't think about retention" classes of bug).

`surface diff` from earlier drafts is **dropped from v0.3**; bidirectional
refinement between two evolving specs is research‑grade and out of scope.

## 13. Escape hatch

`tla { … }` blocks may declare **operators only**. They must not redefine
any name the compiler generates (`Init`, `Next`, `Spec`, `Map`, the names
of surface/substrate actions). The compiler emits the contents into a
separate `<Module>_user.tla` module that the generated module `EXTENDS`,
and rejects any identifier that shadows a generated name.

Use sparingly. Anything inside `tla { … }` is invisible to the doc
generator.

## 14. Notable v0.10 omissions (deliberate)

- **Channel semantics enums** (`unordered | at_least_once | exactly_once`).
  Model channels as ordinary components with explicit nondeterminism for
  reordering or duplication.
- **Attacker inheritance** (`attacker B extends A`). Compose by copy/paste.
- **Module value parameters** at the module level. Use `replicate` for
  per‑instance components (§7.2), `extern` for module‑level configuration.
- **Cross‑module liveness composition.** Parents may not state
  `eventually` properties that depend on child liveness; this requires
  assume/guarantee fairness, deferred (target v0.11).
- **Confidentiality / 2‑safety attackers.** See §10.
- **`surface diff`.** See §12.
- **Inter‑instance messaging in `replicate`** within one component
  (`Edge[i].sends to Edge[j]`). Use a third component (a bus) or a
  shared channel via `compose`.
- **`compose` of more than peer composition.** v0.5 `compose` joins partial
  substrates that target the same surface; *hierarchical* composition
  (parent module surface refined jointly by sub‑module surfaces) is
  separate and remains the v0.4 `module`/sub‑module story.
- **Real (wall-clock) time.** v0.10 introduces *symbolic* time via
  `freshness: bounded(epochs=n)` and `stale_while_revalidate`; an
  "epoch" is a count of substrate propagation steps, not seconds.
  Wall-clock time remains parked (no real-time attackers, no
  duration arithmetic at check time).
- **Author-extensible derivation rules.** v0.9 made the rule set
  closed (compiler-internal). User-defined rules are deferred until
  the built-in catalog is stable — likely never; the closure is
  intentional language design.
- **Confidence levels / probability.** Surface is a discrete-state
  formal language; probabilistic refinement is out of scope.

### 14.1 PL review (v0.10) findings parked for v0.11

The v0.10 staff PL review (see `docs/reviews/PL_REVIEW_V010.md`) raised three
findings v0.10 deliberately defers:

- **A formal label calculus for retention / information flow.** §6.5
  and the R-RETENTION-FLOW / R-INFO-FLOW rules specify a conservative
  syntactic dependency analysis; the PL review proposed a typed
  label lattice (`ephemeral < transactional < … < secret` with PII
  classes). v0.11 target: spec the lattice; specify operator
  join/meet rules; spec the choice between explicit-flow-only vs
  implicit-flow analysis. v0.10 commits to **explicit flow only**;
  implicit flows (a guard reading a secret) are *not* a violation.
- **Slots as an effect row.** Mandatory slots have effect-system
  shape; v0.10 keeps them as syntactic blocks for docs friendliness.
  A future revision may unify them with action types and present
  `defaults` as row-polymorphic elaboration. Not a behaviour change
  if done.
- **Stronger discharge vs acknowledgement separation in §15.3.** v0.10
  permits `because:` reasons on high-severity acknowledgements;
  the PL review noted this risks proof-by-comment. v0.11 may
  distinguish *discharge* (typechecker-recognised counter-evidence)
  from *acknowledgement* (acceptance of risk that appears in emitted
  docs/proof artefacts).

## 15. Static obligations — the consequence-inference pass (v0.9; expanded v0.10)

This section specifies Surface's consequence-inference pass.
v0.9 shipped the framework + 6 representative rules; v0.10
expands the catalog to ~12 rules across the dimensions
covered in §6.4.

### 15.1 What the pass does

After the slot pass (§6.4) and before refinement (§7), the compiler
runs a fixpoint over a closed set of **derivation rules**. Each rule
has the shape

```
premise(s) over declared facts  →  obligation(kind, args)
```

An *obligation* is a fact the user must either **acknowledge** (in an
`acknowledged { … }` block at the substrate or compose level — §15.3)
or **discharge** by adding a stronger declaration. The pass fails if
any derived obligation is neither acknowledged nor discharged.

This is the language-side answer to "the compiler should tell me that
my declaration enables behavior I didn't intend." Compared to the slot
pass (which catches *missing* declarations), the obligation pass
catches *implied* consequences of the declarations that are present.

### 15.1.5 Fact schema and complexity (v0.10, per PL review)

The obligation pass is a least-fixpoint computation over a **finite,
fully-declared fact universe**, followed by post-fixpoint queries
that produce obligation records. The schema below is normative; an
implementation that derives or queries beyond it is non-conformant.

**Base facts (extracted from declarations):**

```
action(A)                                   -- A is a surface action name
action_slot(A, slot, value)                 -- one fact per slot per action
substrate(S)
component(S, C)                             -- component C declared in substrate S
realizes(A, S, C, action)                   -- which substrate action realises A
internal_actn(S, C, action)
channel(S_from, S_to, C_chan)               -- a channel between two substrates
                                            --   (S_from = S_to when intra-substrate)
sends_on(S, C, action, C_chan, msg)
receives_on(S, C, action, C_chan, msg)
ack_path(C_chan_req, C_chan_ack)            -- request channel paired with ack channel
cross_visible(S, aux)
writes_to_cv(S, C, action, aux)
reads_from_cv(S, C, action, aux)
authn(S, A, kind)                           -- kind ∈ {param.<x>, Comp.<f>, system}
field(F)
field_label(F, retention_class)             -- §6.5 state-field retention
field_derived(F)                            -- §6.6 derived state
field_owner(F, S)                           -- partial substrate ownership
emits_in(A, event_name, field_dep_set)      -- which fields' values an emit depends on
epoch(S, eps_name, advances_set, covers_set) -- §7.2.4
freshness_uses(A, eps_name, kind, k)        -- per action freshness slot
```

**Derived facts (least fixpoint, monotone Horn rules):**

```
depends_on(S, S')        :- reads_from_cv(S, _, _, aux), cross_visible(S', aux), S != S'.
depends_on(S, S')        :- channel(S, S', _).
depends_on(S, S'')       :- depends_on(S, S'), depends_on(S', S'').
declared_avail(A, V)     :- action_slot(A, availability, V).
closure_avail(A, weakest_of {V' | realizes(A, S, _, _), depends_on(S, S'), action_slot_of_substrate(S', availability_default, V')}).
expr_dep(F, F')          :- (extracted from AST; see §6.5 retention propagation rules)
secret_flow(A, event)    :- emits_in(A, event, deps), F in deps, field_label(F, secret).
…
```

**Obligation queries (post-fixpoint):**

Each rule in §15.4 is a query of the form
`obligation(<kind>, args) :- <fact pattern>` plus a severity tag.

**Complexity claim:**

Let `N = |declarations|` (linear in source length). Each base-fact
predicate has ≤ `N` tuples (some are bounded by `|fields|` or
`|substrates|`, both `≤ N`). The derived `depends_on` is the
transitive closure of a graph with ≤ `|substrates|` nodes, computed
in `O(|substrates|³)` worst case (`O(|substrates|² + edges)` with
DAG-friendly algorithms). AST dependency extraction is linear in
the AST size. The whole obligation pass is **polynomial in `N`**
(typically near-linear), terminates, and produces a bounded set of
obligation records.

**Monotonicity, carefully stated.** *Derivation* is monotone:
adding base facts can only add derived facts. *Reported unhandled
obligations* are **not** monotone in source size — a new
declaration that discharges an obligation (e.g. adding `idempotent
by(...)` discharges R-REPLAY-AMP) removes that obligation from the
report. The fixpoint over base + derived facts is monotone; the
final report is monotone *given a fixed discharge set*.



### 15.2 Obligation kinds (closed enum, v0.10)

```
obligation kind ::=
    availability_depends_on(<Component>)
  | availability_consistency(<action>, <declared>, <closure>)   -- v0.10
  | availability_channel_class(<action>, <channel>)              -- v0.10
  | trust_transitive(<Component>)
  | information_flow(<from_field>, <to_field>)
  | pii_anon(<action>, <field>)                                  -- v0.10
  | write_conflict(<field>)
  | replay_amplification(<action>)
  | retention_propagation(<source_field>, <derived_field>)
  | actor_view_leak(<observable>, <actor_var>)
  | derived_write(<field>, <action>)                             -- v0.10
  | freshness_channel(<action>, <channel>)                       -- v0.10
```

This list is closed in v0.10. New kinds enter the language only via a
versioned spec change.

### 15.3 The `acknowledged { … }` block (refined v0.10)

A substrate or compose block may declare:

```
substrate <S> realizes <Surface> {
  …
  acknowledged {
    -- LIST-VALUED obligations (multiple components per kind):
    availability_depends_on : [
      ControlPlane                because: "purge fanout depends on ctrl plane",
      SignatureVerifier           because: "signed URL verification",
    ]

    trust_transitive : [
      SignatureVerifier           because: "owner-key authority; rotation via Ops runbook",
    ]

    -- MAP-VALUED obligations (one entry per offending field/action):
    write_conflict : {
      acked : serialized_by(ControlPlane.InvalidationQueue)
              because: "operator rollback supersedes in-flight acks"
    }

    -- SINGLE-ACTION obligations: form is `{ <action> : <resolution>,
    -- because: "…" }`. The resolution slot enumerates per-rule legal
    -- discharges (e.g. R-REPLAY-AMP allows `dedupe_key(<arg>)` or
    -- `idempotent_via_state`).
    replay_amplification : {
      complete_invalidation : idempotent_via_state
                              because: "completed invalidations stay in pending until ackd"
    }
  }
}
```

Rules (v0.10):

1. `because:` is **per-entry** for list-valued obligations and **per
   key** for map-valued obligations. Multiple entries on one line are
   illegal; one entry per line.
2. **High-severity obligations require** `because:`; medium-severity
   may omit it (the docs projection notes a "no reason given"). Empty
   reasons are an error.
3. **Cross-substrate duplicate acknowledgements.** The same obligation
   may be acknowledged by both composed substrates; the entries must
   *agree* (same resolution and a non-conflicting `because:`).
   Disagreement is `E_ACK_DISAGREEMENT(<kind>, <args>)`.
4. **`E_ACK_ORPHAN` is a warning in v0.10**, not an error. Authors
   may proactively acknowledge an obligation the rule catalog hasn't
   codified yet (e.g. an availability dependency derived by author
   intuition). The compiler emits `W_ACK_NO_RULE(<kind>, <args>)` so
   reviewers can decide whether to open a TODO for a new rule or
   delete the acknowledgement.
5. **Co-located with the obligation source.** Acknowledgements live
   inside the substrate (or compose) that the obligation pass derived
   the obligation *from*. The compiler's report tells you where.

### 15.4 Rule catalog (v0.10 — ~12 rules; framework remains closed)

**R-AVAIL-READ.** If a partial substrate `S` reads a `cross_visible`
auxiliary owned by sibling substrate `T`, derive
`availability_depends_on(T)` for `S`. *Severity: medium.* Discharge:
acknowledge in `S.acknowledged`, or restructure the spec so `S` does
not read across the boundary.

**R-AVAIL-CHANNEL.** If a compose declares a channel from `S` to `T`,
derive `availability_depends_on(T)` for `S`. *Severity: medium.* The
derivation is **on the substrate**, not on the surface action's slot.
The slot interaction is covered separately by R-AVAIL-CONSISTENCY.

**R-AVAIL-CONSISTENCY (v0.10).** For each surface action with
declared `availability: <decl>`, the compiler computes the *closure
availability* as the weakest `availability` slot among the substrates
the action transitively depends on (per R-AVAIL-READ / R-AVAIL-CHANNEL).
If `<decl>` is stricter than the closure (e.g. `critical` declared,
closure is `best_effort`), derive `availability_consistency(<action>,
<decl>, <closure>)`. *Severity: high.* Discharge: weaken `<decl>`,
strengthen a substrate's slot, or acknowledge with `because:`.

**R-AVAIL-CHANNEL-CLASS (v0.10).** If a surface action with
`availability: critical` is realised through a substrate channel
declared `at_most_once`-style (i.e. an explicit `internal` drop
action exists on the channel) or through a substrate with declared
`availability: best_effort`, derive
`availability_channel_class(<action>, <channel>)`. *Severity: high.*

**R-TRUST-PARAM-AUTH.** If `authentication { surface_actor of … =
param.<x> }` and the action's `auth_channel` set contains any of
`bearer_token | signed_request | capability_url`, derive
`trust_transitive(<the_signing_authority>)`. *Severity: high.*
Discharge requires a `because: "<reason>"`; the canonical reason
names the authority and its key-management story.

**R-WRITE-CONFLICT.** If a `cross_visible` aux variable is written by
both composed substrates, derive `write_conflict(<field>)`. *Severity:
high.* Discharge requires naming a resolution: `serialized_by:
<Component>`, `last_writer_wins`, `crdt(<merge_op>)`, or
`forbidden_concurrent` (the latter requires a compose-level
`realizes when …` ruling out the race).

**R-REPLAY-AMP.** If an action has `idempotency: at_most_once` or
`at_least_once` AND its `then` block contains an `emit` of an event
classified by §6.4 `observability` as `broadcast(...)`, derive
`replay_amplification(<action>)`. *Severity: medium.* Discharge:
acknowledge or change to `idempotency: idempotent[ by(...) ]`.

**R-RETENTION-FLOW.** If `retention: pii(class=<c>, …)` data is read
into a `then` block and written to a surface state field whose value
is itself read by an action with a weaker `retention` slot, derive
`retention_propagation(<source>, <derived>)`. *Severity: high.*
Discharge: either tighten the derived action's `retention` slot or
acknowledge the flow with an explicit `because:` (typical reason:
"masked at projection — see <observable> definition").

**R-INFO-FLOW (v0.10).** If a state field declared `retention:
pii(class=<c>, …)` is read directly into a non-anonymous observable
or event whose readers include an actor not authorised to see class
`<c>` data, derive `information_flow(<source>, <sink>)`. *Severity:
high.* Discharge: project through a coarsening function or
acknowledge with `because:`.

**R-PII-ANON (v0.10).** If an `auth_channel: { anonymous, … }` action
reads a state field with `retention: pii(...)` in its `when`/`raises`
guard (i.e. the access decision depends on PII), derive
`pii_anon(<action>, <field>)`. *Severity: medium.* This catches the
"anonymous endpoint reads PII-derived configuration" pattern called
out in R7.

**R-ACTOR-VIEW-LEAK.** For every `observable for u: <Actor> … = <body>`,
if `<body>` reads a free actor variable `<v> ≠ u` that is not
derivable from `u` via reachable state, derive
`actor_view_leak(<obs>, <v>)`. *Severity: high.* **No acknowledgement
form**: this is a hard error, the user must rewrite the observable.

**R-DERIVED-WRITE (v0.10).** If an action body assigns to a surface
state field declared `derived from …` (§6.6), derive
`derived_write(<field>, <action>)`. *Severity: high. No
acknowledgement form*: this is an `E_DERIVED_ASSIGN` static error.
Listed in the catalog so the obligation report cross-references the
fix site.

**R-FRESHNESS-CHANNEL (v0.10).** If a surface action declared
`freshness: strong` is realised through a substrate channel without
synchronous acknowledgement (no `receives Ack` returning before the
action commits), derive `freshness_channel(<action>, <channel>)`.
*Severity: high.* Discharge: weaken the freshness slot, add a
synchronous ack, or acknowledge with `because:` (typical reason:
"strong consistency provided by underlying datastore, not the
channel").

### 15.5 What `--obligations` reports

For each derived obligation, the report includes:

1. The rule that derived it (`R-AVAIL-READ`, etc.).
2. The specific declarations that triggered it (file:line of each
   premise).
3. Severity.
4. Whether it is currently acknowledged, discharged, or unhandled.
5. For unhandled high-severity obligations, the message is an *error*
   and blocks codegen. For unhandled medium-severity obligations,
   the message is a *warning* by default; the project can opt into
   `--obligations=strict` to promote them to errors.
6. (v0.10) For proactive `acknowledged` entries without a matching
   rule derivation, a `W_ACK_NO_RULE` warning is included with the
   surrounding source location.

The pass is **monotonic**: adding declarations only adds obligations.
This makes it cheap to re-run on every save.

### 15.6 Why the catalog is closed (v0.9)

A user-extensible rule system would be more flexible but would
fragment the language: two organisations' Surface specs would
no longer mean the same thing. The closed catalog is the
language; "is this rule sensible" is a versioned design decision
made through the same R-round process that shaped every other
construct. v0.10 expanded the catalog from 6 to ~12 rules; v1.0
will freeze the framework, but the catalog will remain
language-versioned.


