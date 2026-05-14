---
title: Modules & Zoom
description: How Surface lets you "zoom in" from user-facing system to gateway to control plane to business logic, with each level checkable independently.
show_table_of_contents: true
---
# Surface — Modules & Zoom

> Companion to [`language-spec.md`](language-spec.md) §2. Read
> [`overview.md`](overview.md) first for the two-layer pitch.

This is the answer to: *"At the top, the user interacts with 'the system'.
Zoom in, the request hits an API gateway, then the control plane, then a
piece of business logic. Non‑technical readers should still understand each
level."*

A **module** is the unit of zoom. Modules may span multiple files
(see [`language-spec.md`](language-spec.md) §2.1) and they nest by qualified name.

## 1. The shape of a module

A **module** is a self‑contained system: its own actors, events, its own
`surface { … }`, optionally one or more `substrate { … }` blocks. The
substrate of a *parent* module uses *child* modules as components, and a
child module's surface acts as the parent‑facing contract.

```
module Banking          ← top: what the customer experiences
  surface  { ... }
  substrate Production { ... uses APIGateway, ControlPlane, Ledger ... }

  module APIGateway     ← zoom 1
    surface  { ... }    ← what the rest of the system sees of "the gateway"
    substrate Default { ... uses RateLimiter, AuthFilter ... }

    module RateLimiter  ← zoom 2
      surface  { ... }
      substrate TokenBucket { ... }
```

At every level the same two‑layer rule applies: the surface is what callers
(human or service) can observe; the substrate is how it is built. A
non‑technical reader can stop at the surface of any level and have a
complete, consistent story (subject to §4 caveats).

## 2. Syntax

```surface
module <Name>(<TypeParam>: Type, ...)? {

  use Banking.{ AccountId, Customer }   -- import names

  surface { ... }

  substrate <SubName> realizes surface {
    component Db { ... }                          -- inline component
    component Gateway : APIGateway.surface        -- child module as black box

    maps     { ... }
    realizes { ... }
    internal { ... }
    auxiliary { ... }
    fairness ...
    authentication { ... }
  }
}
```

Modules take **type parameters only** at the module level (no value
parameters). Use `replicate Component[id in IDS]` for per-instance
components (see [`language-spec.md`](language-spec.md) §7.2.2) and `extern` for
module-level configuration. Use a
`const` inside the module if you need a configurable bound.

Two key rules:

1. **A component typed as `M.surface` is a black box at this level.** The
   parent only knows the child's actions, events, and observable state; it
   cannot see the child's substrate.
2. **Each child module is checked once for safety, in isolation.** Parents
   reason against the child's surface; substrate refinement is local to the
   module that owns it. (Liveness composition: see §4.)

## 3. Worked example — zooming into Banking

### Top level: what the customer sees

```surface
module Banking {
  actor Customer
  event Transferred(from: AccountId, to: AccountId, amount: Nat, by: Customer)

  surface {
    state { balances: Map[AccountId -> Nat] }
    action transfer(f: AccountId, t: AccountId, n: Nat) by c: Customer
      raises {
        NotOwner          when OWNER_OF[f] != c,
        InsufficientFunds when balances[f] < n,
        SameAccount       when f == t,
        ZeroAmount        when n == 0
      }
      then balances[f] -= n
           balances[t] += n
           emit Transferred(from=f, to=t, amount=n, by=c)
  }
}
```

A product manager can read this and stop here.

### Zoom 1: the production substrate

```surface
substrate Production realizes Banking.surface {
  component Gateway : APIGateway.surface
  component Ctrl    : ControlPlane.surface
  component Ledger  : Ledger.surface

  channel ToCtrl    { from Gateway to Ctrl }
  channel ToLedger  { from Ctrl    to Ledger }

  authentication {
    surface_actor of surface.transfer = Gateway.session.user_id
  }

  maps {
    balances = Ledger.balances    -- the user-visible balances ARE the ledger's
  }

  realizes {
    surface.transfer(f, t, n) by Ledger.apply_transfer(f, t, n)
      when Ctrl.last_authorized == Some({f, t, n})
  }

  internal {
    Gateway.*       -- everything inside the gateway is plumbing here
    Ctrl.*          -- and the control plane
    Ledger.audit    -- audit log writes are not user-visible
  }
}
```

Notice that the parent's `internal { … }` lists *child surfaces* whose
actions are plumbing at this level. The child still considers them
first‑class — they're its surface, after all — but from `Banking`'s
viewpoint they don't change what the customer sees.

### Zoom 2: the control plane

```surface
module ControlPlane {
  actor UpstreamCaller
  event Authorized(req: TransferReq, by: UpstreamCaller)
  event Denied(req: TransferReq, by: UpstreamCaller, reason: String)

  surface {
    state {
      last_authorized : Optional[TransferReq]
    }

    action authorize(req: TransferReq) by u: UpstreamCaller
      raises { PolicyDenied when !policy_allows(u, req) }
      then last_authorized := Some(req)
           emit Authorized(req=req, by=u)
  }

  substrate Default realizes surface {
    component Policy   { ... }
    component Audit    { ... }
    component Limits   { ... }

    maps     { last_authorized = Policy.cached_decision }
    realizes { surface.authorize(r) by Policy.commit(r) }
    internal { Audit.*; Limits.*; Policy.lookup }
  }
}
```

A reviewer can now choose their depth:

| Reader              | Reads                                               |
|---------------------|-----------------------------------------------------|
| Customer / PM       | `Banking.surface` only                              |
| Architect           | `Banking.Production` substrate (one zoom)           |
| Control‑plane owner | `ControlPlane.surface` + `ControlPlane.Default`     |
| Security reviewer   | All surfaces (cross‑cutting attacker queries)       |

At every depth the document is the *same artifact* the model checker uses,
so "what the docs say" cannot drift from "what the system does."

## 4. Why this composes — and where it doesn't

### What v0.7 guarantees: safety composition

For **safety** properties (invariants, forbidden traces), the following
composition rule holds and the compiler discharges it automatically:

> If each child substrate refines its child surface for safety, and the
> parent's `maps`/`realizes`/`internal`/`authentication` blocks are valid
> against the child *surfaces*, then the parent substrate refines the
> parent surface for safety.

This is the standard hierarchical refinement pattern and it composes by
trace inclusion. Each layer is checked independently — there is no
cross‑product blow‑up at the parent level for safety.

### What v0.7 does **not** guarantee: liveness composition

Liveness across module boundaries does *not* compose by trace inclusion.
Concretely, if `Banking.Production` claims `eventually Transferred(...)`,
that depends on `ControlPlane` *eventually* authorizing requests, which is
a fairness obligation on the child's surface. v0.7 has no syntax to state
"the child guarantees weak fairness on `authorize`," so the compiler
**rejects** parent‑level `eventually` properties whose proof would require
child liveness. Single‑level liveness (within one module's substrate)
remains supported via `fairness` annotations.

The full assume/guarantee fairness story (à la Abadi–Lamport's *Conjoining
Specifications*) is deferred to v0.8.

### A more practical caveat: `const` configuration tracking

The compiler records the `const` and `extern` configuration used to check
each child module. If a parent uses a child surface with a configuration
that exceeds what the child was checked at (more concurrent callers, larger
key spaces), the compiler emits an error: the child's safety guarantee was
established only at the smaller bound and is not transferable. The fix is
either to re‑check the child at the parent's bound, or to use a symbolic
backend (Apalache) that establishes the guarantee parametrically.

## 5. Naming, imports, visibility

```
use Other.Module                 -- pull a sibling module into scope
use Other.Module.{ Foo, Bar }    -- specific names
module Inner private { ... }     -- visible only inside the enclosing module
```

`private` modules are useful for refactoring a substrate without leaking
new public surface. The compiler enforces that no `surface` outside the
parent references a `private` module's names.
