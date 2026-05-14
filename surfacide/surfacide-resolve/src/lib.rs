//! Module graph, scopes, and name resolution for Surface.
//!
//! M2 scaffolding: ingest parsed `ModuleFile`s into a `Project`,
//! group by module-header name, surface duplicate / shape diagnostics.
//! Full scope resolution lands when the CST→AST converter completes.

use indexmap::IndexMap;
use surfacide_ast::{Decl, FileId, FileRegistry, ModuleFile, Project};
use surfacide_diag::{Diagnostic, ErrorKind};

#[derive(Debug, Default)]
pub struct Resolved {
    pub project: Project,
    pub diagnostics: Vec<Diagnostic>,
}

/// Build a [`Project`] from a set of parsed module files.
///
/// Surfaces duplicate-surface-block / duplicate-action-name issues
/// across the union of files for each module.
pub fn build_project(parsed: Vec<(FileId, ModuleFile)>, files: FileRegistry) -> Resolved {
    let mut project = Project::new();
    project.files = files;
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    for (file_id, module) in parsed {
        project.ingest_file(file_id, module);
    }

    // Cross-file invariants:
    //   - At most one `surface { … }` block per module.
    //   - Duplicate action names within a module's surface block.
    for (_key, module) in &project.modules {
        let mut surface_count = 0usize;
        let mut surface_spans = Vec::new();
        let mut action_seen: IndexMap<String, surfacide_ast::Span> = IndexMap::new();

        for decl in &module.decls {
            if let Decl::Surface(s) = decl {
                surface_count += 1;
                surface_spans.push(s.span);
                // Iterate both `actions` AND `internal_actions` so the
                // duplicate check catches `action X` vs `internal_action
                // X` AND `internal_action X` vs `internal_action X`.
                // (self-review should-fix #3)
                let all_action_names = s
                    .actions
                    .iter()
                    .map(|a| (&a.name, false))
                    .chain(s.internal_actions.iter().map(|ia| (&ia.action.name, true)));
                for (name, is_internal) in all_action_names {
                    if let Some(prev_span) = action_seen.insert(name.name.clone(), name.span) {
                        diagnostics.push(
                            Diagnostic::error(
                                ErrorKind::DuplicateActionName,
                                format!(
                                    "{} `{}` declared more than once in module `{}`",
                                    if is_internal { "internal_action" } else { "action" },
                                    name.name, module.name
                                ),
                                name.span,
                            )
                            .with_label(prev_span, "previous declaration here"),
                        );
                    }
                }
            }
        }

        if surface_count > 1 {
            for s in &surface_spans[1..] {
                diagnostics.push(
                    Diagnostic::error(
                        ErrorKind::DuplicateSurfaceBlock,
                        format!(
                            "module `{}` has {} `surface {{ … }}` blocks; at most one allowed",
                            module.name, surface_count
                        ),
                        *s,
                    )
                    .with_label(surface_spans[0], "first declared here"),
                );
            }
        }
    }

    Resolved { project, diagnostics }
}

/// Wrapper kept for forward-compatibility with the old CLI signature.
/// Prefers [`build_project`] which threads the file registry.
pub fn resolve(_project: &Project) -> Resolved {
    Resolved::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use surfacide_ast::{Ident, ModuleHeader, QualifiedName, Span};
    use surfacide_ast::surface::{ActionDecl, EffectBlock, SurfaceBlock};
    use surfacide_ast::decl::{ActorBinder, Decl, ModuleFile};

    fn id(name: &str) -> Ident {
        Ident::new(name, Span::synthetic())
    }

    fn empty_action(name: &str) -> ActionDecl {
        ActionDecl {
            name: id(name),
            params: Vec::new(),
            return_ty: None,
            actor: ActorBinder { name: id("u"), actor_ty: id("User"), span: Span::synthetic() },
            when_pre: None,
            raises: Vec::new(),
            slots: Vec::new(),
            body: EffectBlock { stmts: Vec::new(), span: Span::synthetic() },
            doc: None,
            span: Span::synthetic(),
            is_internal: false,
        }
    }

    fn module(name: &str, decls: Vec<Decl>) -> (FileId, ModuleFile) {
        (
            FileId(0),
            ModuleFile {
                header: ModuleHeader {
                    name: QualifiedName::new(vec![id(name)]),
                    private: false,
                    doc: None,
                    span: Span::synthetic(),
                },
                uses: Vec::new(),
                decls,
                span: Span::synthetic(),
            },
        )
    }

    fn surface_with(actions: Vec<ActionDecl>) -> Decl {
        Decl::Surface(SurfaceBlock {
            state: Vec::new(),
            init: Vec::new(),
            fairness: Vec::new(),
            properties: Vec::new(),
            defaults: None,
            actions,
            internal_actions: Vec::new(),
            observables: Vec::new(),
            span: Span::synthetic(),
            doc: None,
        })
    }

    #[test]
    fn empty_project_is_clean() {
        let r = build_project(Vec::new(), FileRegistry::new());
        assert!(r.diagnostics.is_empty());
        assert_eq!(r.project.modules.len(), 0);
    }

    #[test]
    fn duplicate_action_in_one_module_is_diagnosed() {
        let m = module(
            "M",
            vec![surface_with(vec![empty_action("transfer"), empty_action("transfer")])],
        );
        let r = build_project(vec![m], FileRegistry::new());
        assert!(r.diagnostics.iter().any(|d| d.code.as_str() == "E_DUPLICATE_ACTION_NAME"));
    }

    #[test]
    fn distinct_actions_are_clean() {
        let m = module(
            "M",
            vec![surface_with(vec![empty_action("a"), empty_action("b")])],
        );
        let r = build_project(vec![m], FileRegistry::new());
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    }

    #[test]
    fn two_surface_blocks_per_module_is_diagnosed() {
        let m = module(
            "M",
            vec![surface_with(vec![empty_action("a")]), surface_with(vec![empty_action("b")])],
        );
        let r = build_project(vec![m], FileRegistry::new());
        assert!(r.diagnostics.iter().any(|d| d.code.as_str() == "E_DUPLICATE_SURFACE_BLOCK"));
    }

    /// Self-review should-fix #3: duplicate detection must cover
    /// `internal_action` declarations too.
    #[test]
    fn duplicate_internal_action_is_diagnosed() {
        let ia = surfacide_ast::surface::InternalActionDecl {
            action: ActionDecl { is_internal: true, ..empty_action("sync") },
        };
        let m = module(
            "M",
            vec![Decl::Surface(surfacide_ast::surface::SurfaceBlock {
                state: Vec::new(),
                init: Vec::new(),
                fairness: Vec::new(),
                properties: Vec::new(),
                defaults: None,
                actions: vec![empty_action("sync")],
                internal_actions: vec![ia],
                observables: Vec::new(),
                span: Span::synthetic(),
                doc: None,
            })],
        );
        let r = build_project(vec![m], FileRegistry::new());
        assert!(
            r.diagnostics.iter().any(|d| d.code.as_str() == "E_DUPLICATE_ACTION_NAME"),
            "expected duplicate-action diagnostic, got: {:?}",
            r.diagnostics
        );
    }
}

