# Surface

A small formal language for specifying systems by their **surface** (what
users experience) and verifying that any chosen **substrate** (how it's
built) refines that surface.

Surface has its own checker — slot pass, obligation pass, docs emit —
and compiles to TLA+ / PlusCal so TLC and Apalache can do the
refinement and temporal model checking.

```surface
surface {
  state { balances : Map[AccountId -> Nat] }

  action transfer(from: AccountId, to: AccountId, amount: Nat)
    by   c: Customer
    raises { InsufficientFunds when balances[from] < amount }
    then balances[from] -= amount
         balances[to]   += amount
         emit Transferred(from=from, to=to, amount=amount, by=c)
}

substrate SqlMonolith realizes surface {
  component Db { ... }
  maps     { balances = Db.rows }
  realizes { surface.transfer(f, t, n) by Db.commit when ... }
}
```

## Start here

- **[`docs/overview.md`](docs/overview.md)** — the pitch, design principles,
  what a `.surf` file produces.
- **[`examples/url-shortener/`](examples/url-shortener/)** — minimal idiomatic spec.
- **[`examples/twitter/`](examples/twitter/)** — multi-substrate Production compose.
- **[`surfacide/README.md`](surfacide/README.md)** — toolchain build / install / usage.
- **[`website/`](website/)** — Genereto-built docs site (deployed to GitHub Pages).

## Reference

- **[`docs/language-spec.md`](docs/language-spec.md)** — normative spec.
- **[`docs/modules.md`](docs/modules.md)** — modules & "zoom" composition.
- **[`docs/coverage.md`](docs/coverage.md)** — what Surface covers and what it deliberately doesn't (v0.10.1).

## History & status

Status: **draft v0.10.1**. The language has been shaped by 8 rounds of
agent-authored specs + folded feedback.

- [`docs/changelog.md`](docs/changelog.md) — per-version history.
- [`docs/reviews/`](docs/reviews/) — staff PL + tool self-reviews.
- [`TODO.md`](TODO.md) — open backlog.
