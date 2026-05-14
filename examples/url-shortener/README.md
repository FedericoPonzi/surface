# `url-shortener` — small starter example (v0.7)

A complete, deliberately tiny Surface (v0.7) spec for a URL shortener.
The goal is to give a new reader an end-to-end model they can read in
five minutes and use as a template for their own first spec.

If you want the *full* demo (`partial substrate` + `compose` +
`aggregate` + cross-substrate `realizes` + `state_at` + `by stutter`),
read `examples/twitter/` instead.

## Files

| File             | Module          | What's in it                                                                      |
|------------------|-----------------|-----------------------------------------------------------------------------------|
| `surface.surf`   | `UrlShortener`  | Actors, types, events, observables, the `surface { … }` block, two properties     |
| `substrate.surf` | `UrlShortener`  | A single in-memory substrate (`InMemoryStore`) with a refinement mapping          |
| `scenarios.surf` | `UrlShortener`  | Three scenarios: happy path (safety), can't-squat (forbidden), eventually (liveness) |

All three files declare `module UrlShortener` at the top — read
`../website/content/blog/language-spec.md` §2.1 for the multi-file module rules.

## What this models

- An `Owner` registers a short slug pointing to a URL.
- A `Visitor` later visits the slug and is redirected to that URL.
- Slugs are unique (you can't squat someone else's).
- The substrate is just one component (`Store`) holding two maps.

That's it. No auth, no expiry, no analytics, no rate limits.

## v0.7 features used

- **Multi-file modules** — three files, one `module UrlShortener` header each.
- **Typed `event`s** with named fields.
- **`observable`s** — `resolves(s)` is a pure derived view of state.
- **`raises { Name when … }`** — single-mechanism error model.
- **Action return values** — `visit(s) -> Url` returns the target; the
  implicit `Returned<Visit>` event is per §5.1 (we don't re-declare it).
- **`e is X` event-type test** + field access on the narrowed type.
- **`events_before(e)`** event-log helper.
- **`_` wildcard** in `observed` clauses.
- **`fairness weak visit`** at the surface — the liveness scenario needs it.
- **`authentication { surface_actor of … = param.<name> }`** binds
  actor identity from action parameters.
- **Two scenario `kind:`s** — `safety` and `liveness`.

## Features deliberately NOT used here

- `partial substrate … owns { … }` and `compose` — single substrate.
- `replicate` — single component.
- Channels, `sends`/`receives` — no inter-component messaging.
- `auxiliary { history … prophecy … }` — no need.
- `history_predicate` — see Twitter for these.
- Labeled `[branch]` `if/else` — actions are simple enough.
- `attacker` blocks — see Twitter.

For all of those, see `examples/twitter/`.
