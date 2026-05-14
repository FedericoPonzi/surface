---
title: Surfacide — the Surface toolchain
description: Surfacide is the Rust frontend toolchain for Surface — parser, slot pass, obligation pass, and docs emit. What it does, who it's for, how to install, what the diagnostics mean.
show_table_of_contents: true
---

# Surfacide

**Surfacide** is the official frontend toolchain for the Surface
specification language. It's a Rust workspace built on
[tree-sitter](https://tree-sitter.github.io/) for parsing and
[miette](https://crates.io/crates/miette) for diagnostics. The binary
ships as a single `surfacide` command.

> **Status.** Version `0.1.0`. Frontend pipeline (parse → resolve → slot
> pass → obligation pass → docs emit) is shipped. **77 tests pass**
> across the workspace; release "examples-must-pass" gate is green.
> The TLA+/PlusCal codegen backend is not yet implemented — see
> [What's next](#whats-next).

## What it does

The compiler runs five passes; each is independently invokable from the CLI.

| Pass | What it produces |
|------|------------------|
| **Parse** | Syntax errors with source spans (tree-sitter grammar covers the full v0.10.1 language). |
| **Resolve** | Cross-file module graph + name-resolution diagnostics. |
| **Slot pass** | §6.4 mandatory-slot diagnostics: missing / out-of-order / unknown-value / empty-waiver / duplicates. Defaults + `internal_action` elaboration with v0.10.1 precedence. |
| **Cross-slot** | Static cross-cutting checks: `E_DERIVED_ASSIGN`, `E_FRESHNESS_UNDECLARED_EPOCH`, `E_SECRET_FLOW`, `E_ACTOR_VIEW_LEAK`. |
| **Obligation pass** | §15 consequence-inference rules (currently 4 of the planned catalogue: R-AVAIL-CHANNEL, R-WRITE-CONFLICT, R-TRUST-PARAM-AUTH, R-FRESHNESS-CHANNEL). `acknowledged { … }` matching across compose + substrate blocks. |
| **Docs emit** | §6.4.3 markdown projection: per-module file with a boundary-checklist table per action, with `(explicit)` / `(default)` / `(internal-preset)` provenance tags. |

## Who it's for

Anyone writing a `.surf` file. Reviewers reading one. CI gates that
need to fail loudly when a spec drifts.

A typical session is a tight edit-check loop with `surfacide` running
in your terminal next to the editor. The two reviewer-agent rounds
(Opus and GPT-5.5) that stress-tested v0.10.1 both found that having
the checker speak back made the language feel different in kind from
purely-design specification: the "what did I forget?" question
becomes mechanically answerable.

## Install

You need the `surfacide` binary. There's no published release yet —
build from source:

```bash
git clone <repo-url>
cd surface/surfacide
cargo build --release
# binary at target/release/surfacide
install -m 755 target/release/surfacide ~/.local/bin/
```

Rust 1.74+ is required. Tree-sitter is vendored — no external grammar
build step, no Node.js dependency.

To run the test suite (77 passing):

```bash
cargo test --workspace
```

## Usage

```
surfacide parse  <path>                  # syntax-level only
surfacide check  <path>                  # all passes
surfacide check  <path> --slots          # only §6.4 slot pass
surfacide check  <path> --obligations    # only §15 obligation pass
surfacide check  <path> --obligations-strict   # warnings → errors
surfacide check  <path> --resolve        # only name resolution
surfacide emit   docs <path> -o <out>    # boundary-checklist markdown
```

`<path>` is a project directory containing `.surf` files. Surfacide
walks the directory transitively, builds the module graph, and runs
the requested passes.

Diagnostics are rendered via [miette](https://crates.io/crates/miette)
with source spans, line numbers, and help text. Every diagnostic
carries a [stable code](#error--warning-codes) — those codes are part
of the public CLI surface and are asserted on by the golden-file
`trycmd` integration tests.

## Authoring a spec with the checker in the loop

A typical session:

1. **Scaffold a module** with `surface { state { … } action foo() -> … }`.
   Run `surfacide check .`. The slot pass will tell you which of the
   seven mandatory §6.4 slots you've left blank.
2. **Fill the slots.** Defaults are fine for prototypes; explicit values
   are required for anything you want to assert. `surfacide emit docs`
   will tell you which is which via provenance tags.
3. **Add a substrate** (`substrate Foo realizes M.surface { … }`).
   The check pass now runs §15 obligation extraction. Expect warnings
   like `W_AVAILABILITY_CLOSURE_WEAKER` ("substrate availability is
   weaker than the surface promises") or `W_WRITE_CONFLICT`
   ("cross_visible aux is written by multiple substrates").
4. **Decide for each warning**: either *fix the spec* (tighten an
   availability, separate an aux per writer) or *acknowledge with reason*:
   `acknowledged { write_conflict: { acked: serialized_by(EdgeMesh) because: "..." } }`.
   The `because:` is mandatory; an empty waiver is `E_SURFACE_SLOT_WAIVER_EMPTY`.
5. **Re-run.** Warnings should drop to zero (or to a known set of
   proactive acks for rules not yet implemented in this Surfacide
   version, which emit `W_ACK_NO_RULE`).
6. **`surfacide emit docs my-spec/ -o out/`** to project the final
   boundary checklist. The output is intended as the artefact reviewed
   by humans — your spec, but with every slot resolved and tagged.

## Error / warning codes

Codes are stable across versions; the catalogue lives in
`surfacide/surfacide-diag/src/codes.rs`.

| Class | Code | Pass |
|-------|------|------|
| parse | `E_PARSE`, `E_INTERNAL` | syntax |
| resolve | `E_NAME_NOT_FOUND`, `E_NAME_AMBIGUOUS`, `E_PRIVATE_MODULE_ACCESS`, `E_DUPLICATE_SURFACE_BLOCK`, `E_DUPLICATE_ACTION_NAME` | M2 |
| slots | `E_SURFACE_SLOT_MISSING`, `E_SURFACE_SLOT_UNKNOWN_VALUE`, `E_SURFACE_SLOT_ORDER`, `E_SURFACE_SLOT_WAIVER_EMPTY`, `E_SLOT_PRECEDENCE_AMBIGUOUS` | M3 |
| cross-slot | `E_DERIVED_ASSIGN`, `E_DERIVED_NO_PROJECTION`, `E_SECRET_FLOW`, `E_FRESHNESS_UNDECLARED_EPOCH`, `E_ACTOR_VIEW_LEAK`, `E_ACK_DISAGREEMENT` | M3 |
| obligations | `W_AVAILABILITY_CLOSURE_WEAKER`, `W_TRUST_PARAM_AUTH`, `W_FRESHNESS_CHANNEL`, `W_WRITE_CONFLICT`, `W_ACK_NO_RULE`, `E_OBLIGATION_UNHANDLED` (strict mode) | M4 |
| advisory | `W_BRANCH_UNLABELLED`, `W_LIVENESS_NO_FAIRNESS` | various |

`--obligations-strict` promotes every `W_*` to a hard error. Useful in
CI; not the default because most warnings are addressed via
acknowledgement, not code change.

## Workspace layout

```
surfacide/
  surfacide-ast/           AST types + spans + provenance
  surfacide-diag/          miette wrappers + stable code catalogue
  surfacide-syntax/        tree-sitter binding + CST→AST
  surfacide-resolve/       module graph + scopes
  surfacide-check/         §6.4 slot pass + cross-slot consistency
  surfacide-obligations/   §15 fact extraction + rule pass
  surfacide-docs/          markdown projection
  surfacide-cli/           the `surfacide` binary
tree-sitter-surface/       grammar.js + vendored generated parser
examples/                  symlinks to repo examples
```

`surfacide-ast` depends on nothing internal. Each crate downstream
depends only on crates above it; the CLI depends on all of them;
nothing depends upward.

## Testing

- **Unit tests** live alongside each crate (`#[cfg(test)] mod tests`).
- **CLI integration tests** are golden-file
  [`trycmd`](https://crates.io/crates/trycmd) sessions under
  `surfacide-cli/tests/trycmd/`. They assert on stdout, stderr, and
  exit code — diagnostic shape is part of the public API.
- **Examples-must-pass gate** (`cargo test --test examples_compile`)
  runs the real parser against every committed `.surf` example.

## What's next

Tracked in [`TODO.md`](https://github.com) and the project's
checkpoints. Headlines:

- **More obligation rules.** The §15 catalogue has 12 planned rules;
  4 are live. R-AVAIL-READ, R-INFO-FLOW, R-RETENTION-PROPAGATION,
  R-DERIVED-WRITE are next.
- **Tighter ack matching.** Per-key vs per-kind ack discharge for
  high-severity obligations (currently kind-only for
  trust/freshness — flagged in the v0.11 design queue).
- **TLA+ / PlusCal codegen.** The biggest unimplemented piece. The
  spec is written assuming this backend; Surfacide produces the
  diagnostics today, the model-checking backend is the v0.2 milestone.
- **Language Server Protocol.** Inline diagnostics in editors. Tree-sitter
  + the stable code catalogue make this mechanical once the obligation
  catalogue is fuller.
