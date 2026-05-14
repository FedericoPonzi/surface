---
title: Surface
description: A refinement-based spec language. Specify systems by their surface (what users experience) and verify that any chosen substrate (how it's built) refines that surface.
add_title: false
show_table_of_contents: false
---

<section class="hero">
  <h1>Surface</h1>
  <p class="tagline">
    A small formal language for specifying systems by their <strong>surface</strong>
    (what users experience) and verifying that any chosen <strong>substrate</strong>
    (how it's built) refines that surface.
  </p>
  <div class="cta">
    <a class="btn btn-primary" href="tutorial.html">15-minute tutorial</a>
    <a class="btn btn-secondary" href="overview.html">Read the overview</a>
    <a class="btn btn-secondary" href="surfacide.html">Get the toolchain</a>
  </div>
</section>

<section class="featured-code">
  <h3>One file, two layers</h3>

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

</section>

<h2 class="section-heading">Why Surface</h2>

<section class="card-grid">
  <div class="card">
    <h3>Two layers, one file</h3>
    <p>The user-visible contract and the implementation live side by side. Drift between them becomes mechanically detectable, not a meeting topic.</p>
    <a href="overview.html">Read more →</a>
  </div>
  <div class="card">
    <h3>Mandatory boundaries</h3>
    <p>Every action declares seven coverage slots — idempotency, auth_channel, retention, rate_limit, observability, availability, freshness. Silence is not a valid design decision; <code>waived: "&lt;reason&gt;"</code> is.</p>
    <a href="coverage.html">See coverage manifest →</a>
  </div>
  <div class="card">
    <h3>Counter-examples, not obligations</h3>
    <p>When refinement fails, you get a concrete narrated trace, not a stuck proof. Surface compiles to PlusCal/TLA+ so TLC and Apalache do the heavy lifting.</p>
    <a href="language-spec.html#12-tooling">How it's checked →</a>
  </div>
</section>

<h2 class="section-heading">What one Surface file produces</h2>

<div class="what-table">

| Artifact | How |
|----------|-----|
| **Boundary checklist** | The seven mandatory action slots fail closed; `surfacide check --slots` errors on omissions. |
| **Static obligations** | A consequence-inference pass derives obligations from your declarations (e.g. "you read this cross-substrate aux, so your availability depends on that substrate"). Each must be acknowledged or discharged. |
| **Formal model** | Compiles to PlusCal/TLA+; TLC/Apalache check invariants, temporal properties, and refinement. |
| **Living documentation** | A boundary-checklist per action, rendered from the same file the checker uses, so it cannot drift. |
| **Security review** | `actor`, `attacker`, and the `auth_channel` slot drive integrity-reachability checks: *can role X cause effect Y by any chain of allowed actions?* |
| **Use cases** | `scenario` blocks render to Markdown and are mechanically checked for reachability. |

</div>

<h2 class="section-heading">The toolchain</h2>

<section class="card-grid">
  <div class="card">
    <h3>Surfacide</h3>
    <p>Rust frontend toolchain — parser, slot pass, obligation pass, docs emit. Single binary, miette-style diagnostics with stable error codes. The compiler talks back so the "what did I forget?" question becomes mechanical.</p>
    <a href="surfacide.html">Read the Surfacide guide →</a>
  </div>
  <div class="card">
    <h3>Stable diagnostic codes</h3>
    <p>Every error and warning carries a stable code (<code>E_SURFACE_SLOT_MISSING</code>, <code>W_WRITE_CONFLICT</code>, …). Asserted in golden tests. CI-friendly. Searchable.</p>
    <a href="surfacide.html#error--warning-codes">See the catalogue →</a>
  </div>
  <div class="card">
    <h3>Edit-check loop</h3>
    <p>Run <code>surfacide check .</code> in a terminal next to your editor. The slot pass tells you which of seven mandatory boundaries you've left blank; the obligation pass tells you which cross-cutting consequences need an explicit decision.</p>
    <a href="surfacide.html#authoring-a-spec-with-the-checker-in-the-loop">Authoring workflow →</a>
  </div>
</section>

<p style="text-align:center; color: var(--muted); margin-top: 32px;">
  Surface is <strong>experimental</strong>. The language has been shaped by 8 rounds of agent-authored
  specifications + folded feedback (R1 → R9). See the <a href="changelog.html">changelog</a> for
  per-version history, and <a href="https://github.com/" target="_blank" rel="noopener">the repo</a>
  for the toolchain and examples.
</p>
