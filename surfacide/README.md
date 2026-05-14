# Surfacide

A Rust frontend toolchain for the **Surface** specification language.

Surface (see `../website/content/blog/language-spec.md` at the repo root) is a refinement-based
DSL for describing system *boundaries*: what's promised, what's allowed,
and what's deliberately undefined. Surfacide is the checker that turns
those promises into mechanical diagnostics — the goal is that a reviewer
running `surfacide check my-spec/` learns exactly what's been declared
about a system, what's been silently elided, and what cross-cutting
obligations the elisions induce.

## Status

Frontend through to obligation pass + docs emit is shipped:

- Tree-sitter grammar covers the full v0.10.1 language.
- CST → typed AST conversion for every v0.10.1 construct.
- Module-graph builder with cross-file duplicate detection.
- **§6.4 slot pass**: missing / out-of-order / unknown-value / empty-waiver
  / duplicate diagnostics; defaults + internal_action elaboration with
  v0.10.1 precedence; `E_DERIVED_ASSIGN` analysis on action bodies.
- **§15 obligation pass** with four live rules:
  R-AVAIL-CHANNEL, R-WRITE-CONFLICT, R-TRUST-PARAM-AUTH, R-FRESHNESS-CHANNEL.
  `acknowledged { … }` matching across compose + substrate blocks.
  Promotable to error via `--obligations-strict`.
- **§6.4.3 docs emit**: per-module markdown with a boundary-checklist
  table per action plus `(explicit)` / `(default)` / `(internal-preset)`
  provenance tags.
- **68 tests pass** (1 ignored); release "examples-must-pass" gate is green.

Tracked roadmap: [`TODO.md`](../TODO.md) at the repo root.

## Install

Build from source — Surfacide vendors the generated tree-sitter parser, so
the only build dependency is Rust (1.74+ recommended).

```bash
cd surfacide
cargo build --release
```

The binary lands at `target/release/surfacide`. Symlink or copy it onto
your `$PATH` for daily use:

```bash
install -m 755 target/release/surfacide ~/.local/bin/
```

To run the test suite:

```bash
cargo test --workspace
```

## Usage

```
surfacide parse  <path>           # syntax-level only
surfacide check  <path>           # all passes
surfacide check  <path> --slots         # only §6.4 slot pass
surfacide check  <path> --obligations   # only §15 obligation pass
surfacide check  <path> --obligations-strict   # warnings → errors
surfacide check  <path> --resolve       # only name resolution
surfacide emit   docs <path> -o <out>   # boundary-checklist markdown
```

`<path>` is a project directory containing `.surf` files. Surfacide reads
every `.surf` file under that directory transitively, builds the module
graph, and runs the requested passes.

Diagnostics are rendered via [`miette`](https://crates.io/crates/miette)
with source spans. Every diagnostic carries a stable code (see below) —
those codes are part of the public CLI surface and are asserted on by the
golden-file `trycmd` tests.

## Authoring a spec with the checker in the loop

The intended workflow is a tight edit-check loop with the binary running
in your terminal next to your editor. The R-round reviewer agents (Opus
and GPT-5.5) both found that having the checker speak back made the
language feel different in kind from purely-design specification: the
"what did I forget?" question becomes mechanically answerable.

A typical session:

1. **Scaffold a module** with `surface { state { … } action foo() -> … }`.
   Run `surfacide check .`. The slot pass will tell you which of the
   13 mandatory §6.4 slots you've left blank.
2. **Fill the slots.** Defaults are fine for prototypes; explicit values
   are required for anything you want to assert. The provenance tags in
   `surfacide emit docs` will tell you which is which.
3. **Add a substrate** (`substrate Foo realizes M.surface { … }`).
   The check pass now runs §15 obligation extraction. Expect warnings
   like `W_AVAILABILITY_CLOSURE_WEAKER` ("substrate availability is
   weaker than the surface promises") or `W_WRITE_CONFLICT`
   ("cross_visible aux is written by multiple substrates").
4. **Decide for each warning**: either *fix the spec* (tighten an
   availability, separate an aux per writer) or *acknowledge with
   reason*: `acknowledged { write_conflict: { acked: serialized_by(EdgeMesh) because: "..." } }`.
   The `because:` is mandatory; an empty waiver is `E_SURFACE_SLOT_WAIVER_EMPTY`.
5. **Re-run.** Warnings should drop to zero (or to a known set of
   proactive acks for rules not yet implemented in this Surfacide
   version, which emit `W_ACK_NO_RULE`).
6. **`surfacide emit docs my-spec/ -o out/`** to project the final
   boundary checklist. The output is intended as the artefact reviewed
   by humans — your spec, but with every slot resolved and tagged.

Two worked specs live in the repo at `examples/`:

- `examples/url-shortener/` — minimal idiomatic v0.10.1 spec
  (1 warning: `visit` uses `param.v` for actor identity → R-TRUST-PARAM-AUTH).
- `examples/twitter/` — multi-substrate Production compose
  (0 warnings; clean acknowledgement of `availability_depends_on`).

## Error / warning codes

Codes are stable across versions; the catalogue lives in
`crates/surfacide-diag/src/codes.rs`.

| Class | Code | Pass |
|---|---|---|
| parse | `E_PARSE`, `E_INTERNAL` | syntax |
| resolve | `E_NAME_NOT_FOUND`, `E_NAME_AMBIGUOUS`, `E_PRIVATE_MODULE_ACCESS`, `E_DUPLICATE_SURFACE_BLOCK`, `E_DUPLICATE_ACTION_NAME` | M2 |
| slots | `E_SURFACE_SLOT_MISSING`, `E_SURFACE_SLOT_UNKNOWN_VALUE`, `E_SURFACE_SLOT_ORDER`, `E_SURFACE_SLOT_WAIVER_EMPTY`, `E_SLOT_PRECEDENCE_AMBIGUOUS` | M3 |
| cross-slot | `E_DERIVED_ASSIGN`, `E_DERIVED_NO_PROJECTION`, `E_SECRET_FLOW`, `E_FRESHNESS_UNDECLARED_EPOCH`, `E_ACTOR_VIEW_LEAK`, `E_ACK_DISAGREEMENT` | M3 |
| obligations | `W_AVAILABILITY_CLOSURE_WEAKER`, `W_TRUST_PARAM_AUTH`, `W_FRESHNESS_CHANNEL`, `W_WRITE_CONFLICT`, `W_ACK_NO_RULE`, `E_OBLIGATION_UNHANDLED` (strict mode) | M4 |
| advisory | `W_BRANCH_UNLABELLED`, `W_LIVENESS_NO_FAIRNESS` | various |

## Workspace layout

```
crates/
  surfacide-ast/           AST types + spans + provenance
  surfacide-diag/          miette wrappers + stable code catalogue
  surfacide-syntax/        tree-sitter binding + CST→AST
  surfacide-resolve/       module graph + scopes
  surfacide-check/         §6.4 slot pass + cross-slot consistency
  surfacide-obligations/   §15 fact extraction + rule pass
  surfacide-docs/          markdown projection
  surfacide-cli/           the `surfacide` binary
tree-sitter-surface/       grammar.js + vendored generated parser
examples/                  symlinks into repo examples
```

`surfacide-ast` depends on nothing internal. Each crate downstream depends
only on crates above it in the workspace tree; the CLI may depend on all
of them; nothing depends upward.

## Test layout

- **Unit tests** live alongside each crate.
- **CLI integration tests** are golden-file [`trycmd`](https://crates.io/crates/trycmd)
  sessions under `crates/surfacide-cli/tests/trycmd/`. They assert on
  stdout, stderr, and exit code — diagnostic shape is part of the public
  API.
- **Examples-must-pass gate**: `cargo test --test examples_compile` runs
  the real parser against every committed example.
