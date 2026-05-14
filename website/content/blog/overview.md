---
title: Overview
description: Surface — what it is, why two layers, and what one .surf file produces.
show_table_of_contents: true
---
# Surface

> A small formal language for specifying systems by their **surface** (what users
> experience) and verifying that any chosen **substrate** (how it is actually
> built) faithfully realizes that surface.

Surface has its own toolchain — a slot-coverage pass and a static-obligation
pass — and **compiles to PlusCal / TLA+** as a backend so that the TLC and
Apalache model checkers can handle exhaustive refinement and temporal
checking. TLA+ is one backend among the things Surface produces from a single
file; the language itself stays small, readable, and close to how product
people, security reviewers, and engineers already talk about a system.

---

## 1. The core idea

When you describe a system, two different things deserve names:

1. **Surface behavior** — the contract experienced from the outside.
   *“If Alice has 100 credits and transfers 30 to Bob, Alice ends up with 70,
   Bob with +30, nobody else changes, and the operation either succeeds or has
   no effect.”*
2. **Substrate behavior** — the moving parts that make the surface true.
   *“There is an `accounts` table, a `ledger` table, a write‑ahead log, two
   services communicating over a queue …”*

Most specification languages let you write one of these. Surface insists on
**both, side by side**, with a checked relationship between them:

```
                 ┌────────────────────────┐
   user / docs   │       SURFACE          │   ← one canonical definition
   e2e tests     │  actors, actions,      │
   threat model  │  observable state,     │
                 │  invariants, scenarios │
                 └───────────▲────────────┘
                             │  refines
                 ┌───────────┴────────────┐
   architecture  │      SUBSTRATE(s)      │   ← swap MySQL for DynamoDB,
   ops, runbooks │  components, internal  │     monolith for microservices,
                 │  state, messages,      │     etc.
                 │  refinement mapping    │
                 └────────────────────────┘
```

A single `surface { … }` block can have **many** `substrate { … } realizes …`
blocks. The compiler proves (or refutes) that each substrate refines the
surface. Switching from MySQL to DynamoDB is a question you answer with the
checker, not a meeting.

---

## 2. What you can do with one Surface file

| Artifact                  | How Surface produces it                                                |
|---------------------------|------------------------------------------------------------------------|
| **Boundary checklist**    | Every `action` declares **seven** mandatory coverage slots (idempotency, auth_channel, retention, rate_limit, observability, availability, freshness). `surfacide check --slots` errors on omissions. |
| **Static obligations**    | A consequence-inference pass derives obligations from your declarations (e.g. "you read this cross-substrate aux, so your availability depends on that substrate"). Each must be acknowledged or discharged. |
| **Formal model**          | Compiles to PlusCal/TLA+ (Surface's primary backend); TLC/Apalache check invariants, temporal props, and refinement. |
| **Use cases / docs**      | `scenario` blocks render to Markdown; the checker proves they're reachable and correct. The slot pass adds a per-action boundary checklist to the docs projection. |
| **E2E test skeletons**    | Each `scenario` can emit Gherkin / Playwright / pytest stubs.          |
| **Security review**       | `actor`, `attacker`, and the `auth_channel` slot drive integrity-reachability checks: *can role X cause effect Y by any chain of allowed actions?* |
| **Implementation gate**   | Each `substrate` is checked to refine the surface. Failed refinement → counter‑example trace. |
| **Living documentation**  | Docs are extracted from the same file the checker uses, so they cannot drift. |

---

## 3. Design principles

1. **Two layers, one file.** The surface and at least one substrate live
   together so that drift is mechanically detectable.
2. **Boring on the page, formal underneath.** Indentation + a handful of
   keywords. No `□`, `◇`, `∃` in user‑facing syntax (they are emitted into the
   generated TLA+).
3. **The language has its own checker; TLA+ is a backend.** The slot pass and
   the obligation pass run in the Surface frontend with Rust-style local
   errors. TLA+/PlusCal codegen is the primary backend for refinement and
   temporal model checking, not Surface's semantics.
4. **Actors are first‑class.** Every action is performed *by* someone with
   *permissions*. This is what enables security questions to be asked
   declaratively.
5. **Observability is explicit.** A surface state field is something the user
   can in principle observe; substrate state is hidden by default. The
   refinement mapping is the bridge. Actor-relative observability uses
   `observable for <Actor>` (v0.9).
6. **Boundaries are required, not optional.** Every action's seven coverage
   slots (§6.4) must be filled or explicitly waived in source. Silence is
   not a valid design decision; `waived: "<reason>"` is.
7. **Scenarios are executable prose.** A `scenario` is simultaneously: a piece
   of documentation, a model‑checker query ("this trace exists / is
   impossible"), and a test fixture.
8. **Counter‑examples are the primary output.** When something fails,
   you get a concrete narrated trace, not a proof obligation.

---

## 4. File layout at a glance

```surface
system Banking

actor Customer
actor Admin

event Transferred(from: AccountId, to: AccountId, amount: Nat, by: Customer)

surface {
  state {
    balances : Map[AccountId -> Nat]
  }

  property conservation {
    always sum(balances.values) == INITIAL_TOTAL
  }

  action transfer(from: AccountId, to: AccountId, amount: Nat)
    by   c: Customer
    raises {
      NotOwner          when OWNER_OF[from] != c,
      InsufficientFunds when balances[from] < amount,
      SameAccount       when from == to
    }
    then balances[from] -= amount
         balances[to]   += amount
         emit Transferred(from=from, to=to, amount=amount, by=c)
}

scenario "Alice pays Bob" kind: safety {
  given balances == { A: 100, B: 0 }
  when  Alice does transfer(A, B, 30)
  then  balances == { A: 70, B: 30 }
        observed Transferred(from=A, to=B, amount=30, by=Alice) by Alice
}

substrate SqlMonolith realizes surface {
  component Db { ... }
  authentication { surface_actor of surface.transfer = Db.txn.who }
  maps     { balances = Db.rows }
  realizes { surface.transfer(f, t, n) by Db.commit when ... }
  internal { Db.begin; Db.abort }
}

substrate DynamoSagas realizes surface {
  component AccountSvc { ... }
  component Saga       { ... }
  channel   Bus        { from Saga to AccountSvc }
  auxiliary { prophecy outcome_of : Map[SagaId -> Outcome] := * }
  maps      { balances = AccountSvc.table + (committed-pending) - (aborted-pending) }
  realizes  { surface.transfer(f, t, n) by AccountSvc.debit(f, n) when ... }
  internal  { Saga.*; AccountSvc.credit; AccountSvc.compensate_debit; Bus.deliver }
}
```

The next files walk through the language feature by feature
([`language-spec.md`](language-spec.md)), through worked examples
([`../../../examples/`](../../../examples/), starting with `url-shortener/`), through
modular composition / "zoom" ([`modules.md`](modules.md)), and through
deliberate coverage and exclusions ([`coverage.md`](coverage.md)).
