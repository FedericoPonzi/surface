# `twitter` — the canonical end-to-end Surface example (v0.10)

The full v0.10 demo: a small social network (post, follow, protect,
fetch) modeled with **two peer substrates joined by `compose`**, a
cross-substrate channel, a cross-substrate `realizes` override, an
`aggregate` mapping, replicated components, history predicates, an
attacker block, labeled `if/else` branches, and proper return-value
handling.

If you want a gentle intro instead, read `examples/url-shortener/`
first — that example is deliberately tiny.

## Files

| File             | Module     | What's in it                                                                      |
|------------------|------------|-----------------------------------------------------------------------------------|
| `surface.surf`   | `Twitter`  | Actors, types, events, two top-level `history_predicate`s, observables, `surface { … }`, properties, attacker |
| `data.surf`      | `Twitter`  | `partial substrate PostStore owns { posts, next_seq }` with `replicate UserAccount[u]` and `aggregate using union_set` |
| `graph.surf`     | `Twitter`  | `partial substrate FollowGraph owns { follows, protected }` with single `Index` component, explicit `noop` stutter action |
| `compose.surf`   | `Twitter`  | `compose Production = PostStore + FollowGraph` with cross-substrate channel + cross-substrate `realizes` overrides for `fetch_post` |
| `scenarios.surf` | `Twitter`  | Four scenarios across `safety` / `liveness`                                       |

All files declare `module Twitter` at the top — read
`../website/content/blog/language-spec.md` §2.1 for multi-file module rules.

## What this models

- A `User` can `post` a message. Each post id is `(author, sequence)`
  to guarantee uniqueness across replicas.
- A `User` can `follow` and `unfollow` other users.
- A `User` can mark themselves `protected`. Protected users' posts are
  visible only to followers.
- A `User` can `fetch_post(post_id)` and either receive the content or
  be denied based on the visibility rule.

That's it. No retweets, no replies, no DMs, no rate limits.

## v0.7 features exercised

| Feature                                       | Where                                                |
|-----------------------------------------------|------------------------------------------------------|
| Multi-file modules                            | All five `.surf` files declare `module Twitter`     |
| Typed `event` declarations                    | `surface.surf` — Posted, Followed, Disclosed, …      |
| Action return values (implicit `Returned<X>`) | `post -> PostId`, `fetch_post -> Optional[Content]`  |
| `observable name(args): T = expr`             | (none — v0.9 migrated to actor-relative)              |
| `observable for u: <Actor>` (v0.9)            | `following()`, `i_am_protected()`, `visible_posts()`, `can_see(pid)` |
| Mandatory action coverage slots (v0.9)        | Every action carries the seven slots (v0.10 added freshness) |
| `derived from` surface state (v0.10)          | `surface.surf` — `posts = derived from aggregate UserAccount[u].my_posts using union_set` |
| `defaults { … }` slot block (v0.10)           | `surface.surf` — defaults block fills auth_channel/retention/rate_limit/availability/freshness |
| `private` state-field modifier (v0.10)        | `surface.surf` — follows, protected |
| `freshness` slot (v0.10)                      | per-action; e.g. fetch_post inherits `eventual` |
| `raises { Name when G }`                      | `follow` (`CantFollowSelf`), `fetch_post` (`NotFound`) |
| `if … then [label] … else [label]`            | `follow`, `unfollow`, `set_protected`, `fetch_post`  |
| `param.<name>` in `authentication`            | Both substrates                                      |
| `replicate Component[id in IDS]`              | `data.surf` — `UserAccount[u in USERS]`              |
| `aggregate ... using union_set` (v0.7)         | `data.surf` — `posts = aggregate UserAccount[u].my_posts using union_set` |
| Map comprehension over `replicate`            | `data.surf` — `next_seq = { u -> UserAccount[u].my_seq | u in USERS }` |
| `partial substrate ... owns { … }`            | Both substrates                                      |
| `compose <Name> = A + B`                      | `compose.surf`                                       |
| Cross-substrate channel + completeness         | `compose.surf` declares `FollowChangedBus`; `data.surf` has matching `receives FollowChanged from FollowChangedBus` |
| `sends Msg(...) to <ChannelName>`             | `graph.surf` — sends `FollowChanged` to the bus      |
| Cross-substrate `realizes` override (strengthen) | `compose.surf` — `fetch_post[allow]` / `[deny]`    |
| `by stutter` for no-effect branches (v0.7)     | `compose.surf` — `follow[redundant]`/etc.            |
| Top-level `history_predicate`                  | `surface.surf` — `is_following_at`, `is_protected_at` |
| `state_at(e)` helper (v0.7)                    | `surface.surf` — both history predicates             |
| `e is X && e.field == …` event-type test       | History predicates and properties                    |
| `events_before(e)`                             | `no_fabrication` property                            |
| `aggregate ... using union_set` (v0.7 rename, was `using union`) | `data.surf` — `posts` map |
| `_` wildcard in `observed`                     | Scenarios                                            |
| `attacker` block + `eventually emits`          | `surface.surf` — `NonFollowerSnoop`                  |
| Labeled liveness scenario `requires_in:`       | Scenario 4                                           |
| Tuple keys (`Map[(User, User) -> Bool]`)       | `surface.surf` and substrates                        |

## Known limitations and design choices

- **Surface state is "global".** `posts`, `follows`, `protected` are
  surface state, but no single user can observe all of them. This is
  the [TODO.md P1-7](../../TODO.md) limitation: the two-layer model
  strains where state is genuinely actor-relative. We mitigate by
  exposing per-user views as `observable`s. Resolving this needs the
  `observable for <Actor>` form proposed in P1-7 (deferred to v0.8).
- **Implicit `Returned<X>` events** are emitted by `return` per spec
  §5.1.1 (v0.7); explicit declarations are forbidden. Substrate
  realizing actions also return a value, satisfying refinement.
