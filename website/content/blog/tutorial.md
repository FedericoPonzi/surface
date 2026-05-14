---
title: Tutorial — Your first Surface spec
description: Build a working URL-shortener spec in 15 minutes. From "what is this" to "the checker just told me what I forgot," step by step.
show_table_of_contents: true
---

# Tutorial: your first Surface spec

This tutorial gets you from zero to a working spec the
[Surfacide](surfacide.html) checker is happy with. We'll build a tiny
URL shortener in five steps. By the end you'll have:

- A **surface** (the user-visible contract).
- A **substrate** (a real implementation with its own component state).
- A clean run of `surfacide check` — including one obligation warning
  the checker found and that you decided how to handle.

You need ~15 minutes and the `surfacide` binary on your `$PATH`. Get
it from the [Surfacide page](surfacide.html#install) (build from
source, ~1 minute).

> **Mental model.** A `.surf` file describes a system on **two layers**.
> The *surface* is what users experience — actions, state they can
> observe, what's allowed to happen. The *substrate* is one
> implementation of that surface, with its own internal state and
> actions, mapped back to the surface via `maps { … }` and
> `realizes { … }` clauses. The checker today verifies the
> *frontend* obligations (slot coverage, cross-cutting consequences);
> a TLA+/PlusCal codegen for full refinement model-checking is on the
> roadmap. Read [Overview](overview.html) for the longer pitch; you
> don't need it to follow this tutorial.

## Step 1 — A module and the actors

Make a new directory `tut/` with one file `tut/spec.surf`:

```surface
module Tut

actor Owner    -- registers short links
actor Visitor  -- follows them

type Slug = String
type Url  = String
```

Run the checker:

```
$ surfacide check tut/
surfacide: 1 file(s) checked, no diagnostics
```

You've declared two **actors**. Every action in Surface is performed
*by* an actor — that's what lets the security story (who can do what
to whom) be checked instead of asserted.

`type Slug = String` is a type alias, just for readability.

## Step 2 — Your first action, and the slot pass

Add a `surface { … }` block. Inside it, declare the state and one
action:

```surface
surface {
  state {
    links : Map[Slug -> Url]
  }

  init {
    links := {}
  }

  action create(s: Slug, target: Url)
    by    o: Owner
    raises { SlugTaken when s in links.keys }
    then  links[s] := target
}
```

Re-run `surfacide check tut/`. You'll see seven errors, one per
missing slot:

```
error[E_SURFACE_SLOT_MISSING]: action `create` is missing required slot `idempotency`
   ╭─[tut/spec.surf:...]
   ╰────
  help: add `idempotency: <value>` or set a project-wide default
```

…and the same shape for `auth_channel`, `retention`, `rate_limit`,
`observability`, `availability`, `freshness`.

**This is the point of Surface.** Seven cross-cutting decisions are
made mandatory for every action:

| Slot | "What's the answer to…" |
|------|-------------------------|
| `idempotency` | If a client retries, what happens? |
| `auth_channel` | How is the caller authenticated? |
| `retention` | What gets logged or stored? |
| `rate_limit` | What stops abuse? |
| `observability` | What does the caller learn? |
| `availability` | Strong consistency or best-effort? |
| `freshness` | How stale can reads be? |

Silence is not a valid design decision. Either fill the slot or
explicitly waive it: `auth_channel: waived: "internal-only POC"`.

## Step 3 — Defaults

Filling seven slots per action is loud. Add a `defaults { … }` block
**inside `surface { }`** — and add one event + one constant the slots
will reference. Replace your file with:

```surface
module Tut

actor Owner    -- registers short links
actor Visitor  -- follows them

type Slug = String
type Url  = String

const MINUTE : Duration

event Created(slug: Slug, target: Url, by: Owner)

surface {
  defaults {
    auth_channel  : session
    retention     : transactional
    rate_limit    : per_actor(100, per=MINUTE)
    observability : caller_only(Created)
    availability  : critical
    freshness     : strong
  }

  state {
    links : Map[Slug -> Url]
  }

  init {
    links := {}
  }

  action create(s: Slug, target: Url)
    by    o: Owner
    raises { SlugTaken when s in links.keys }
    idempotency : idempotent by(s, target, o)
    -- six other slots inherited from defaults
    then  links[s] := target
          emit Created(slug=s, target=target, by=o)
}
```

`surfacide check tut/` should now be clean. Each action only
mentions the slots it overrides; the rest come from `defaults`.

> **What `defaults` actually does.** It applies to *every* action in
> this surface. A per-action slot line wins over a default; the
> defaults win over `internal_action` presets. The boundary
> checklist (`surfacide emit docs`) tags each effective slot value
> with `(explicit)`, `(default)`, or `(internal-preset)` so a reviewer
> can see exactly where each decision came from.

## Step 4 — A second action with per-action overrides

Add a second event and the `visit` action. Note the per-action slot
lines that override the defaults:

```surface
event Visited(slug: Slug, target: Url, by: Visitor)

-- inside `surface { }`, after `create`:
action visit(s: Slug) -> Url
  by    v: Visitor
  raises { NotFound when s not in links.keys }
  idempotency   : idempotent
  auth_channel  : { anonymous, capability_url, signed_request }
  retention     : ephemeral
  observability : caller_only(Visited)
  freshness     : eventual
  -- availability inherited (critical)
  then  emit Visited(slug=s, target=links[s], by=v)
        return links[s]
```

Three things to notice:

- **`auth_channel: { anonymous, capability_url, signed_request }`** —
  a *set* of allowed authentication channels. The action accepts any
  of them; the substrate (next step) decides per channel. This is the
  channel-agnostic shape that makes URLs fan out across "open",
  "signed", and "capability" cases without a Cartesian explosion.
- **`retention: ephemeral`** — visits aren't logged. (Compare `create`,
  which is `transactional`.)
- **`freshness: eventual`** — a brief replication lag on a `visit`
  is fine; we don't need `strong`.

`surfacide check tut/` is still clean. Each override is now an
explicit, reviewable design decision.

## Step 5 — A real substrate, and the obligation pass fires

Below the `surface { }` block, add an in-memory substrate. This is a
**real implementation**: a `Store` component with its own state and
two actions, plus `maps { }` and `realizes { }` clauses that connect
it back to the surface.

```surface
substrate InMemoryStore realizes Tut.surface {

  component Store {
    state {
      table : Map[Slug -> Url]
    }

    init {
      table := {}
    }

    action put(s: Slug, target: Url, o: Owner)
      when not (s in table.keys)
      then table[s] := target
           emit Created(slug=s, target=target, by=o)

    action lookup(s: Slug, v: Visitor) -> Url
      when s in table.keys
      then emit Visited(slug=s, target=table[s], by=v)
           return table[s]
  }

  authentication {
    surface_actor of surface.create = param.o
    surface_actor of surface.visit  = param.v
  }

  maps {
    links = Store.table
  }

  realizes {
    surface.create(s, target) by Store.put
      when Store.put.s == s && Store.put.target == target

    surface.visit(s) by Store.lookup
      when Store.lookup.s == s
  }
}
```

Re-run the checker. You'll now see a warning:

```
warning[W_TRUST_PARAM_AUTH]: action `visit` carries trust-bearing
auth_channel and uses `param.v` for actor identity; R-TRUST-PARAM-AUTH
   ╭─[tut/spec.surf:...]
   ╰────
  help: add `acknowledged { trust_transitive: [<SigningAuthority>
        because: "..."] }` ...
```

**This is the obligation pass.** Surfacide noticed: `visit` accepts
`signed_request` and `capability_url` — auth channels that imply some
authority *vouched* for the caller's identity. But your substrate
just trusts `param.v` (whatever the caller said it was). There's no
declared signing authority. That's a real design gap — either change
the substrate's authentication model, or **acknowledge** the gap
explicitly:

```surface
substrate InMemoryStore realizes Tut.surface {
  -- ...component, authentication, maps, realizes as above...

  acknowledged {
    trust_transitive : [
      EdgeKey because: "POC accepts client-supplied visitor IDs; production must front Store with a signed-URL gateway that mints v from the claim."
    ]
  }
}
```

Re-run. Clean:

```
$ surfacide check tut/
surfacide: 1 file(s) checked, no diagnostics
```

The acknowledgement is part of the spec: it shows up in the
boundary-checklist docs (`surfacide emit docs tut/ -o tut-docs/`),
survives review, and the `because:` reason is part of the audit
trail. The checker won't let you write `because: ""` — the reason is
mandatory.

## What you just learned

In ~15 minutes:

- A `.surf` file has two layers — **surface** (contract) and
  **substrate** (implementation with its own state, mapped back via
  `maps { }` + `realizes { }`).
- Every action declares **seven boundary slots**. The compiler
  enforces this.
- **`defaults { }`** lets you set project-wide values; per-action
  lines override them. Each override is an explicit design decision
  the docs projection will tag with provenance.
- The **obligation pass** spots cross-cutting consequences ("you
  trust this thing, but the auth model doesn't justify it"). You
  either fix the spec or write an `acknowledged { because: "..." }`
  with a real reason.
- The whole loop is mechanical: write some, run `surfacide check`,
  read the diagnostic, fix the spec or acknowledge the gap, repeat.

## Where to next

- **[Overview](overview.html)** — the longer pitch and design
  principles, now that the loop makes sense.
- **[`examples/url-shortener/`](https://github.com)** — the full
  worked spec you just rebuilt, plus a `scenarios.surf` showing
  scenario blocks (executable use cases, model-checked once the
  TLA+/PlusCal backend lands).
- **[`examples/twitter/`](https://github.com)** — the same idea at
  multi-substrate scale (partial substrates, compose, fairness,
  attackers).
- **[Language spec](language-spec.html)** — the normative reference.
  Skip §1–§4 unless you hit a question; the §6 surface block, §7
  substrate block, and §15 obligations are where the value is.
- **[Surfacide page](surfacide.html)** — every CLI flag and every
  diagnostic code, when you want to know what the checker is telling
  you.

> **Honest disclaimer.** Surface is a young, experimental language.
> Today's checker verifies frontend obligations — slot coverage,
> cross-cutting consequences, structural consistency — and produces
> a boundary-checklist docs projection. It does **not yet** drive
> TLA+/PlusCal model-checking; that's the v0.2 milestone for
> Surfacide. So when this tutorial says "the checker is happy," it
> means the frontend is happy. Refinement model-checking is coming.
