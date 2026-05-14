//! tree-sitter binding and CST → AST conversion for Surface.
//!
//! Exposes a [`Parser`] that wraps the vendored tree-sitter-surface
//! grammar and produces a typed AST + diagnostics.
//!
//! The grammar itself lives at `../../tree-sitter-surface/`; the
//! generated `parser.c` is compiled in via `build.rs`.

#![allow(unused)]

use surfacide_ast::{FileId, FileRegistry, Span};
use surfacide_diag::{Diagnostic, ErrorKind};
use tree_sitter::{Language, Node};

mod cst_to_ast;

mod ts {
    extern "C" {
        pub fn tree_sitter_surface() -> super::Language;
    }
}

/// The Surface tree-sitter language handle.
pub fn language() -> Language {
    unsafe { ts::tree_sitter_surface() }
}

/// A Surface parser. Each call to [`Parser::parse`] returns the raw CST
/// (via tree-sitter) plus any syntax-error diagnostics found by walking
/// the tree for `(ERROR …)` and `(MISSING …)` nodes.
///
/// CST → typed AST conversion lands in `cst_to_ast.rs` (next).
pub struct Parser {
    inner: tree_sitter::Parser,
}

impl Parser {
    pub fn new() -> Self {
        let mut inner = tree_sitter::Parser::new();
        inner
            .set_language(&language())
            .expect("tree-sitter-surface language load");
        Self { inner }
    }

    pub fn parse(&mut self, file: FileId, source: &str) -> ParseResult {
        let tree = match self.inner.parse(source, None) {
            Some(t) => t,
            None => {
                return ParseResult {
                    module: None,
                    diagnostics: vec![Diagnostic::error(
                        ErrorKind::ParseError,
                        "tree-sitter aborted while parsing this file",
                        Span::new(file, 0, source.len() as u32),
                    )],
                    cst_root_kind: None,
                };
            }
        };

        let root = tree.root_node();
        let mut diags = Vec::new();
        collect_errors(root, file, source, &mut diags);

        // Run CST → AST conversion. The converter is tolerant; even
        // when parts of the AST aren't yet populated (stubbed
        // sub-converters), the diagnostic stream tells the caller.
        let (module, cvt_diags) = cst_to_ast::convert_module_file(root, source, file);
        diags.extend(cvt_diags);

        ParseResult {
            module,
            diagnostics: diags,
            cst_root_kind: Some(root.kind().to_string()),
        }
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// Walk the CST and record `(ERROR ...)` and `(MISSING ...)` nodes as
/// diagnostics.
fn collect_errors(node: Node, file: FileId, source: &str, out: &mut Vec<Diagnostic>) {
    if node.is_error() || node.is_missing() {
        let span = Span::new(file, node.start_byte() as u32, node.end_byte() as u32);
        let snippet = source
            .get(node.start_byte()..node.end_byte().min(node.start_byte() + 40))
            .unwrap_or("");
        let msg = if node.is_missing() {
            format!("missing `{}` here", node.kind())
        } else {
            format!(
                "syntax error near `{}`",
                snippet.split_whitespace().next().unwrap_or(snippet)
            )
        };
        out.push(Diagnostic::error(ErrorKind::ParseError, msg, span));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_errors(child, file, source, out);
    }
}

#[derive(Debug, Default)]
pub struct ParseResult {
    pub module: Option<surfacide_ast::decl::ModuleFile>,
    pub diagnostics: Vec<Diagnostic>,
    /// CST root node kind name. `Some(...)` if parsing produced a tree
    /// (even with errors). `None` only if tree-sitter aborted entirely.
    pub cst_root_kind: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) {
        let mut p = Parser::new();
        let r = p.parse(FileId(0), src);
        assert!(
            r.diagnostics.is_empty(),
            "expected clean parse of `{}`, got: {:#?}",
            src.lines().next().unwrap_or(src),
            r.diagnostics
        );
    }

    fn parse_err(src: &str) {
        let mut p = Parser::new();
        let r = p.parse(FileId(0), src);
        assert!(
            !r.diagnostics.is_empty(),
            "expected at least one diagnostic for `{}`",
            src.lines().next().unwrap_or(src)
        );
    }

    #[test]
    fn parses_minimal_module() {
        parse_ok("module X\n");
    }

    #[test]
    fn parses_actor_and_event() {
        parse_ok("module M\nactor User\nevent Foo(x: Nat)\n");
    }

    // The `use Module.{A, B}` form is in spec §2 but no v0.10-era
    // example exercises it, so the grammar fixer didn't include it.
    // Re-enable when name resolution (M2) needs cross-module imports.
    #[test]
    #[ignore = "grammar gap: `use` imports — defer to M2 when needed"]
    fn parses_use_import() {
        parse_ok("module M\nuse Other.{Bar, Baz}\n");
    }

    #[test]
    fn parses_extern_and_const() {
        parse_ok(
            "module M\nextern USERS : Set[User]\nconst MINUTE : Duration\n",
        );
    }

    #[test]
    fn parses_subtype_actor() {
        parse_ok("module M\nactor Admin extends User\n");
    }

    #[test]
    fn reports_syntax_error() {
        parse_err("module X\n@@@\n");
    }

    #[test]
    fn missing_module_header_is_error() {
        // every .surf file must start with `module <Name>`; the first
        // declaration without a header is rejected.
        parse_err("actor User\n");
    }

    #[test]
    fn line_comments_are_ignored() {
        parse_ok("module M -- a comment\nactor User -- and another\n");
    }

    #[test]
    fn block_comments_are_ignored() {
        parse_ok("module M\n{- a block\n   comment -}\nactor User\n");
    }

    fn parse_module(src: &str) -> surfacide_ast::decl::ModuleFile {
        let mut p = Parser::new();
        let r = p.parse(FileId(0), src);
        assert!(
            r.diagnostics.is_empty(),
            "expected clean parse, got: {:#?}",
            r.diagnostics
        );
        r.module.expect("module should be present")
    }

    #[test]
    fn surface_block_state_and_action() {
        let m = parse_module(
            "module M\nactor User\nevent Created()\nsurface {\n  state { x : Nat }\n  init { x := 0 }\n  action create() by u: User\n    then x := x + 1\n}\n",
        );
        let surf = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Surface(s) => Some(s),
            _ => None,
        }).expect("surface");
        assert_eq!(surf.state.len(), 1);
        assert_eq!(surf.actions.len(), 1);
        assert_eq!(surf.actions[0].name.as_str(), "create");
    }

    #[test]
    fn surface_property_decl() {
        let m = parse_module(
            "module M\nactor User\nsurface { state { x : Nat } init { x := 0 } }\nproperty safe { always true }\n",
        );
        let p = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Property(p) => Some(p),
            _ => None,
        }).expect("property");
        assert_eq!(p.name.as_str(), "safe");
    }

    #[test]
    fn substrate_component_and_channel() {
        let m = parse_module(
            "module M\nactor User\nsurface { state { x : Nat } init { x := 0 } }\nsubstrate Sub realizes M.surface {\n  component C { state { y : Nat } init { y := 0 } }\n  channel Bus { from C to C }\n}\n",
        );
        let sub = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Substrate(s) => Some(s),
            _ => None,
        }).expect("substrate");
        assert_eq!(sub.name.as_str(), "Sub");
        assert_eq!(sub.components.len(), 1);
        assert_eq!(sub.channels.len(), 1);
        assert_eq!(sub.channels[0].name.as_str(), "Bus");
    }

    #[test]
    fn substrate_replicate_block() {
        let m = parse_module(
            "module M\nactor User\nextern USERS : Set[User]\nsurface { state { x : Nat } init { x := 0 } }\nsubstrate Sub realizes M.surface {\n  replicate Account[u: User in USERS] {\n    state { z : Nat }\n    init { z := 0 }\n  }\n}\n",
        );
        let sub = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Substrate(s) => Some(s),
            _ => None,
        }).expect("substrate");
        assert_eq!(sub.replicates.len(), 1);
        assert_eq!(sub.replicates[0].component.name.as_str(), "Account");
    }

    #[test]
    fn compose_block_members() {
        let m = parse_module(
            "module M\nactor User\nsurface { state { x : Nat } init { x := 0 } }\npartial substrate A realizes M.surface owns { x } { component C { state { p : Nat } init { p := 0 } } }\npartial substrate B realizes M.surface owns { x } { component D { state { q : Nat } init { q := 0 } } }\ncompose Top = A + B { }\n",
        );
        let comp = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Compose(c) => Some(c),
            _ => None,
        }).expect("compose");
        assert_eq!(comp.name.as_str(), "Top");
        assert_eq!(comp.members.len(), 2);
        assert_eq!(comp.members[0].as_str(), "A");
    }

    #[test]
    fn scenario_decl_basic() {
        let m = parse_module(
            "module M\nactor User\nsurface { state { x : Nat } init { x := 0 } }\nscenario \"basic\" kind: safety {\n  actors { u: User }\n  given true\n  then true\n}\n",
        );
        let s = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Scenario(s) => Some(s),
            _ => None,
        }).expect("scenario");
        assert_eq!(s.title, "basic");
        assert!(matches!(s.kind, surfacide_ast::scenario::ScenarioKind::Safety));
        assert_eq!(s.actors.len(), 1);
    }

    #[test]
    fn scenario_kind_liveness() {
        let m = parse_module(
            "module M\nactor User\nsurface { state { x : Nat } init { x := 0 } }\nscenario \"live\" kind: liveness {\n  actors { u: User }\n  then true\n}\n",
        );
        let s = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Scenario(s) => Some(s),
            _ => None,
        }).expect("scenario");
        assert!(matches!(s.kind, surfacide_ast::scenario::ScenarioKind::Liveness));
    }

    #[test]
    fn attacker_decl_basic() {
        let m = parse_module(
            "module M\nactor User\nevent E()\nsurface { state { x : Nat } init { x := 0 } }\nattacker A {\n  controls u : User\n  initial true\n  may any action allowed for u\n  goal eventually emits E()\n}\n",
        );
        let a = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Attacker(a) => Some(a),
            _ => None,
        }).expect("attacker");
        assert_eq!(a.name.as_str(), "A");
        assert_eq!(a.controls.name.as_str(), "u");
    }

    #[test]
    fn substrate_internal_block() {
        let m = parse_module(
            "module M\nactor User\nsurface { state { x : Nat } init { x := 0 } }\nsubstrate Sub realizes M.surface {\n  component C { state { y : Nat } init { y := 0 } }\n  internal { }\n}\n",
        );
        let sub = m.decls.iter().find_map(|d| match d {
            surfacide_ast::Decl::Substrate(s) => Some(s),
            _ => None,
        }).expect("substrate");
        assert!(sub.internal.is_some(), "internal block should be parsed");
    }

    #[test]
    fn url_shortener_examples_parse() {
        // Smoke test: walk the url-shortener example dir and parse all
        // .surf files. Catches regressions in the cst→ast lowering even
        // when the higher-level `every_v10_example_parses` integration
        // test isn't run.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/url-shortener");
        // CARGO_MANIFEST_DIR is `<repo>/surfacide/surfacide-syntax`; up
        // two levels = `<repo>/`.
        let mut count = 0;
        for entry in std::fs::read_dir(&root).expect("examples dir") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("surf") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let mut p = Parser::new();
            let r = p.parse(FileId(0), &src);
            assert!(
                r.diagnostics.is_empty(),
                "expected clean parse of {:?}, got: {:#?}",
                path.file_name(),
                r.diagnostics
            );
            count += 1;
        }
        assert!(count > 0, "expected at least one .surf file");
    }

    /// Regression test for self-review must-fix #2: the `type_decl`
    /// grammar field is `value`, not `type`. A previous converter
    /// silently dropped every `type Foo = ...` declaration.
    #[test]
    fn type_alias_is_not_silently_dropped() {
        let src = "module M\ntype Slug = String\ntype Count = Nat\n";
        let mut p = Parser::new();
        let mut files = FileRegistry::new();
        let id = files.add(std::path::PathBuf::from("x.surf"), src);
        let r = p.parse(id, src);
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
        let module = r.module.expect("module parsed");
        let alias_count = module
            .decls
            .iter()
            .filter(|d| matches!(d, surfacide_ast::Decl::TypeAlias(_)))
            .count();
        assert_eq!(alias_count, 2, "expected two type aliases in the AST");
    }
}

