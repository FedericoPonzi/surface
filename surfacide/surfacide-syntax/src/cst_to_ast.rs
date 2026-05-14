//! CST → typed AST conversion.
//!
//! Walks the tree-sitter parse tree and produces a typed [`ModuleFile`]
//! (from `surfacide-ast`). Every AST node carries its source span so
//! later passes can produce diagnostics with code frames.
//!
//! ## Conversion conventions
//!
//! - Every public `cvt_*` function takes a `tree_sitter::Node` and a
//!   `&Cvt` context (source + file id). Returning `Some(T)` indicates
//!   a successfully-converted node; `None` indicates we hit a node we
//!   don't yet handle (rare; the converter is meant to be total over
//!   the grammar).
//! - We emit diagnostics into `Cvt::diags` (a mutable shared buffer) on
//!   shape mismatches. Most "this CST node is missing a required child"
//!   shapes are grammar/converter bugs; we surface them as `E_INTERNAL`
//!   so they don't slip into release silently.
//! - **Spans** come straight from `Node::start_byte()/end_byte()`.
//! - **Field-based access** (`node.child_by_field_name("name")`) is the
//!   preferred way to find a child; we only iterate `named_children()`
//!   for "list of N" patterns (e.g. param lists).

use surfacide_ast::*;
use surfacide_diag::{Diagnostic, ErrorKind};
use tree_sitter::Node;

mod convert_expr;
mod convert_surface;
mod convert_substrate;
mod convert_compose;
mod convert_scenario;
mod convert_attacker;

pub(crate) struct Cvt<'a> {
    pub source: &'a str,
    pub file: FileId,
    pub diags: Vec<Diagnostic>,
}

impl<'a> Cvt<'a> {
    pub fn new(source: &'a str, file: FileId) -> Self {
        Self { source, file, diags: Vec::new() }
    }

    pub fn span(&self, node: Node) -> Span {
        Span::new(self.file, node.start_byte() as u32, node.end_byte() as u32)
    }

    pub fn text(&self, node: Node) -> &'a str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }

    pub fn internal_error(&mut self, node: Node, msg: impl Into<String>) {
        self.diags.push(Diagnostic::error(
            ErrorKind::Internal,
            msg,
            self.span(node),
        ));
    }

    /// Find the first named child whose kind is `kind`.
    pub fn first_child<'b>(&self, node: Node<'b>, kind: &str) -> Option<Node<'b>> {
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == kind {
                return Some(child);
            }
        }
        None
    }

    /// Collect every named child whose kind is in `kinds`.
    pub fn named_children_of<'b>(&self, node: Node<'b>, kinds: &[&str]) -> Vec<Node<'b>> {
        let mut cursor = node.walk();
        let mut out = Vec::new();
        for child in node.named_children(&mut cursor) {
            if kinds.contains(&child.kind()) {
                out.push(child);
            }
        }
        out
    }

    /// Collect every named child regardless of kind (excluding comments
    /// and doc strings).
    pub fn all_named_children<'b>(&self, node: Node<'b>) -> Vec<Node<'b>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .filter(|n| !matches!(n.kind(), "line_comment" | "block_comment"))
            .collect()
    }
}

// ============================================================================
// Top-level entry point: source_file → ModuleFile
// ============================================================================

/// Convert a `source_file` CST node into a [`ModuleFile`].
///
/// Returns `None` (with diagnostics) when the file lacks a `module`
/// header — every `.surf` file must start with one (spec §2).
pub fn convert_module_file(root: Node, source: &str, file: FileId) -> (Option<ModuleFile>, Vec<Diagnostic>) {
    let mut cvt = Cvt::new(source, file);
    let result = convert_module_file_inner(root, &mut cvt);
    (result, cvt.diags)
}

fn convert_module_file_inner(root: Node, cvt: &mut Cvt) -> Option<ModuleFile> {
    if root.kind() != "source_file" {
        cvt.diags.push(Diagnostic::error(
            ErrorKind::Internal,
            format!(
                "expected `source_file` root, got `{}` (parser invariant)",
                root.kind()
            ),
            cvt.span(root),
        ));
        return None;
    }

    let span = cvt.span(root);
    let mut header: Option<ModuleHeader> = None;
    let mut uses: Vec<UseDecl> = Vec::new();
    let mut decls: Vec<Decl> = Vec::new();
    let mut pending_doc: Option<String> = None;

    for child in cvt.all_named_children(root) {
        match child.kind() {
            "doc_string" => {
                let text = doc_string_text(cvt.text(child));
                pending_doc = Some(text);
                continue;
            }
            "module_header" => {
                if header.is_some() {
                    cvt.internal_error(child, "duplicate `module` header in one file");
                    continue;
                }
                header = Some(convert_module_header(child, cvt, pending_doc.take()));
            }
            "use_decl" => {
                if let Some(u) = convert_use_decl(child, cvt) {
                    uses.push(u);
                }
                pending_doc = None;
            }
            "tla_block" => {
                // Parsed but not consumed by checker passes (spec §13).
                pending_doc = None;
            }
            kind => {
                if let Some(decl) = convert_top_level_decl(child, kind, cvt, pending_doc.take()) {
                    decls.push(decl);
                }
            }
        }
    }

    let header = match header {
        Some(h) => h,
        None => {
            cvt.diags.push(Diagnostic::error(
                ErrorKind::ParseError,
                "missing `module <Name>` header (every .surf file requires one — spec §2)",
                span,
            ));
            return None;
        }
    };
    Some(ModuleFile { header, uses, decls, span })
}

fn convert_module_header(node: Node, cvt: &mut Cvt, doc: Option<String>) -> ModuleHeader {
    let span = cvt.span(node);
    let name_node = node
        .child_by_field_name("name")
        .expect("grammar invariant: module_header.name");
    let name = convert_qualified_name(name_node, cvt);
    let private = node.child_by_field_name("visibility").is_some();
    ModuleHeader { name, private, doc, span }
}

fn convert_use_decl(node: Node, cvt: &mut Cvt) -> Option<UseDecl> {
    let span = cvt.span(node);
    // The current grammar doesn't ship a `use_decl` rule in v0.10.1; the
    // node-type schema lists it for forward compatibility. If a future
    // grammar emits it, the structure here matches the spec §2 form
    // `use M.{A, B}`.
    let module_node = cvt.first_child(node, "qualified_name")?;
    let module = convert_qualified_name(module_node, cvt);
    let items: Vec<Ident> = cvt
        .named_children_of(node, &["identifier"])
        .into_iter()
        .map(|n| ident_from_node(n, cvt))
        .collect();
    Some(UseDecl { module, items, span })
}

fn convert_top_level_decl(
    node: Node,
    kind: &str,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<Decl> {
    match kind {
        "type_decl" => convert_type_alias(node, cvt, doc).map(Decl::TypeAlias),
        "actor_decl" => convert_actor(node, cvt, doc).map(Decl::Actor),
        "event_decl" => convert_event(node, cvt, doc).map(Decl::Event),
        "const_decl" => convert_const(node, cvt, doc).map(Decl::Const),
        "extern_decl" => convert_extern(node, cvt, doc).map(Decl::Extern),
        "observable_decl" => convert_observable_decl(node, cvt, doc).map(Decl::Observable),
        "actor_observable_decl" => convert_actor_observable_decl(node, cvt, doc).map(Decl::Observable),
        "history_predicate_decl" => {
            convert_history_predicate_decl(node, cvt, doc).map(Decl::HistoryPredicate)
        }
        "attacker_decl" => convert_attacker::convert_attacker_decl(node, cvt, doc).map(Decl::Attacker),
        "property_decl" => convert_surface::convert_property_decl(node, cvt, doc).map(Decl::Property),
        "scenario_decl" => convert_scenario::convert_scenario_decl(node, cvt, doc).map(Decl::Scenario),
        "surface_block" => convert_surface::convert_surface_block(node, cvt, doc).map(Decl::Surface),
        "substrate_block" => convert_substrate::convert_substrate_block(node, cvt, doc).map(Decl::Substrate),
        "partial_substrate_block" => {
            convert_substrate::convert_partial_substrate_block(node, cvt, doc).map(Decl::PartialSubstrate)
        }
        "compose_block" => convert_compose::convert_compose_block(node, cvt, doc).map(Decl::Compose),
        // Comments are filtered at the source_file traversal level.
        _ => {
            cvt.internal_error(node, format!("unhandled top-level decl kind `{}`", kind));
            None
        }
    }
}

// ============================================================================
// Common low-level helpers
// ============================================================================

pub(crate) fn ident_from_node(node: Node, cvt: &Cvt) -> Ident {
    debug_assert_eq!(node.kind(), "identifier");
    Ident::new(cvt.text(node), cvt.span(node))
}

pub(crate) fn convert_qualified_name(node: Node, cvt: &Cvt) -> QualifiedName {
    debug_assert!(matches!(node.kind(), "qualified_name"));
    let segments: Vec<Ident> = cvt
        .named_children_of(node, &["identifier"])
        .into_iter()
        .map(|n| ident_from_node(n, cvt))
        .collect();
    // The grammar guarantees ≥1 segment.
    if segments.is_empty() {
        // Defensive: synthesise an empty segment so we don't panic.
        return QualifiedName::new(vec![Ident::new("<missing>", cvt.span(node))]);
    }
    QualifiedName::new(segments)
}

/// Extract the textual content of a `doc_string` node, stripping the
/// triple-quote delimiters. The grammar surrounds the body with `"""`.
pub(crate) fn doc_string_text(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix("\"\"\"")
        .and_then(|s| s.strip_suffix("\"\"\""))
        .unwrap_or(trimmed);
    stripped.trim().to_string()
}

// ============================================================================
// Type expressions
// ============================================================================

pub(crate) fn convert_type_expr(node: Node, cvt: &mut Cvt) -> Type {
    // type_expr wraps one of: simple_type | generic_type | qualified_type
    // | record_type | tuple_type | union_type | enum_type
    let span = cvt.span(node);
    let inner = match node.kind() {
        "type_expr" => match cvt.all_named_children(node).into_iter().next() {
            Some(c) => c,
            None => {
                cvt.internal_error(node, "empty type_expr");
                return Type { kind: TypeKind::Named(QualifiedName::new(vec![Ident::new("<empty>", span)])), span };
            }
        },
        _ => node,
    };

    let kind = match inner.kind() {
        "simple_type" => convert_simple_type(inner, cvt),
        "generic_type" => convert_generic_type(inner, cvt),
        "qualified_type" => convert_qualified_type(inner, cvt),
        "record_type" => convert_record_type(inner, cvt),
        "tuple_type" => convert_tuple_type(inner, cvt),
        "union_type" => convert_union_type(inner, cvt),
        "enum_type" => convert_enum_type(inner, cvt),
        other => {
            cvt.internal_error(inner, format!("unhandled type kind `{}`", other));
            TypeKind::Named(QualifiedName::new(vec![Ident::new("<unknown>", span)]))
        }
    };
    Type { kind, span }
}

fn convert_simple_type(node: Node, cvt: &mut Cvt) -> TypeKind {
    let id_node = cvt.first_child(node, "identifier").unwrap_or(node);
    let name = cvt.text(id_node);
    match name {
        "Nat" => TypeKind::Nat,
        "Int" => TypeKind::Int,
        "Bool" => TypeKind::Bool,
        "String" => TypeKind::String,
        "Duration" => TypeKind::Duration,
        _ => TypeKind::Named(QualifiedName::new(vec![Ident::new(name, cvt.span(id_node))])),
    }
}

fn convert_generic_type(node: Node, cvt: &mut Cvt) -> TypeKind {
    let base_node = node
        .child_by_field_name("base")
        .expect("grammar invariant: generic_type.base");
    let base_name = cvt.text(base_node);
    let span = cvt.span(node);

    // Map[K -> V] uses fields key/value; Set[T] / Seq[T] / Optional[T] use
    // a single anonymous type_expr child.
    if let (Some(key_node), Some(val_node)) = (
        node.child_by_field_name("key"),
        node.child_by_field_name("value"),
    ) {
        let k = convert_type_expr(key_node, cvt);
        let v = convert_type_expr(val_node, cvt);
        return match base_name {
            "Map" => TypeKind::Map(Box::new(k), Box::new(v)),
            _ => {
                cvt.internal_error(node, format!("unexpected key/value on generic type `{}`", base_name));
                TypeKind::Named(QualifiedName::new(vec![Ident::new(base_name, cvt.span(base_node))]))
            }
        };
    }

    let inner_types: Vec<Type> = cvt
        .named_children_of(node, &["type_expr"])
        .into_iter()
        .map(|n| convert_type_expr(n, cvt))
        .collect();

    match (base_name, inner_types.len()) {
        ("Set", 1) => TypeKind::Set(Box::new(inner_types.into_iter().next().unwrap())),
        ("Seq", 1) => TypeKind::Seq(Box::new(inner_types.into_iter().next().unwrap())),
        ("Optional", 1) => TypeKind::Optional(Box::new(inner_types.into_iter().next().unwrap())),
        _ => {
            // Treat unknown generics as named for now; resolution may flag.
            cvt.internal_error(node, format!("unhandled generic type `{}` with {} args", base_name, inner_types.len()));
            TypeKind::Named(QualifiedName::new(vec![Ident::new(base_name, cvt.span(base_node))]))
        }
    }
}

fn convert_qualified_type(node: Node, cvt: &mut Cvt) -> TypeKind {
    let qn_node = cvt.first_child(node, "qualified_name").unwrap_or(node);
    TypeKind::Named(convert_qualified_name(qn_node, cvt))
}

fn convert_record_type(node: Node, cvt: &mut Cvt) -> TypeKind {
    let fields: Vec<RecordTypeField> = cvt
        .named_children_of(node, &["record_type_field"])
        .into_iter()
        .map(|f| {
            let span = cvt.span(f);
            let name = f
                .child_by_field_name("name")
                .map(|n| ident_from_node(n, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", span));
            let ty = f
                .child_by_field_name("type")
                .map(|n| convert_type_expr(n, cvt))
                .unwrap_or(Type {
                    kind: TypeKind::Named(QualifiedName::new(vec![Ident::new("<missing>", span)])),
                    span,
                });
            RecordTypeField { name, ty, span }
        })
        .collect();
    TypeKind::Record(fields)
}

fn convert_tuple_type(node: Node, cvt: &mut Cvt) -> TypeKind {
    let elts: Vec<Type> = cvt
        .named_children_of(node, &["type_expr"])
        .into_iter()
        .map(|n| convert_type_expr(n, cvt))
        .collect();
    TypeKind::Tuple(elts)
}

fn convert_union_type(node: Node, cvt: &mut Cvt) -> TypeKind {
    let elts: Vec<Type> = cvt
        .named_children_of(node, &["type_expr"])
        .into_iter()
        .map(|n| convert_type_expr(n, cvt))
        .collect();
    TypeKind::Union(elts)
}

fn convert_enum_type(node: Node, cvt: &mut Cvt) -> TypeKind {
    let variants: Vec<Ident> = cvt
        .named_children_of(node, &["identifier"])
        .into_iter()
        .map(|n| ident_from_node(n, cvt))
        .collect();
    TypeKind::Enum(variants)
}

// ============================================================================
// Simple top-level declarations
// ============================================================================

fn convert_type_alias(node: Node, cvt: &mut Cvt, doc: Option<String>) -> Option<TypeAliasDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    // Grammar names the RHS field `value`; accept legacy `type` too for
    // robustness against future grammar evolutions (self-review #2).
    let ty = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("type"))
        .map(|n| convert_type_expr(n, cvt))?;
    Some(TypeAliasDecl { name, ty, doc, span })
}

fn convert_actor(node: Node, cvt: &mut Cvt, doc: Option<String>) -> Option<ActorDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let extends = node.child_by_field_name("parent").map(|n| ident_from_node(n, cvt));
    Some(ActorDecl { name, extends, doc, span })
}

fn convert_event(node: Node, cvt: &mut Cvt, doc: Option<String>) -> Option<EventDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let fields = if let Some(plist) = cvt.first_child(node, "param_list") {
        convert_param_list(plist, cvt)
            .into_iter()
            .map(|p| EventField { name: p.name, ty: p.ty, span: p.span })
            .collect()
    } else {
        Vec::new()
    };
    Some(EventDecl { name, fields, doc, span })
}

fn convert_const(node: Node, cvt: &mut Cvt, doc: Option<String>) -> Option<ConstDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let ty = node
        .child_by_field_name("type")
        .map(|n| convert_type_expr(n, cvt))?;
    Some(ConstDecl { name, ty, doc, span })
}

fn convert_extern(node: Node, cvt: &mut Cvt, doc: Option<String>) -> Option<ExternDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let ty = node
        .child_by_field_name("type")
        .map(|n| convert_type_expr(n, cvt))?;
    Some(ExternDecl { name, ty, doc, span })
}

pub(crate) fn convert_param_list(node: Node, cvt: &mut Cvt) -> Vec<crate::cst_to_ast::ParamLike> {
    cvt.named_children_of(node, &["param"])
        .into_iter()
        .map(|p| {
            let span = cvt.span(p);
            let name = p
                .child_by_field_name("name")
                .map(|n| ident_from_node(n, cvt))
                .unwrap_or_else(|| Ident::new("<missing>", span));
            let ty = p
                .child_by_field_name("type")
                .map(|n| convert_type_expr(n, cvt))
                .unwrap_or(Type {
                    kind: TypeKind::Named(QualifiedName::new(vec![Ident::new("<missing>", span)])),
                    span,
                });
            ParamLike { name, ty, span }
        })
        .collect()
}

/// Internal helper struct so callers can shape params into either
/// `Param`, `EventField`, etc. without re-parsing.
#[derive(Clone)]
pub(crate) struct ParamLike {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

impl From<ParamLike> for surfacide_ast::decl::Param {
    fn from(p: ParamLike) -> Self {
        Self { name: p.name, ty: p.ty, span: p.span }
    }
}

// ============================================================================
// Observables (regular and actor-relative)
// ============================================================================

pub(super) fn convert_observable_decl_pub(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<ObservableDecl> {
    convert_observable_decl(node, cvt, doc)
}

pub(super) fn convert_actor_observable_decl_pub(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<ObservableDecl> {
    convert_actor_observable_decl(node, cvt, doc)
}

/// Shared `fairness <weak|strong> <target>` converter.
pub(super) fn convert_fairness_decl(node: Node, cvt: &mut Cvt) -> Option<surfacide_ast::substrate::FairnessSpec> {
    use surfacide_ast::substrate::{FairnessSpec, FairnessStrength};
    let span = cvt.span(node);
    let strength = match node
        .child_by_field_name("kind")
        .map(|n| cvt.text(n))
        .unwrap_or("weak")
    {
        "strong" => FairnessStrength::Strong,
        _ => FairnessStrength::Weak,
    };
    let target_node = node.child_by_field_name("target")?;
    let target = parse_fairness_target(target_node, cvt);
    Some(FairnessSpec { strength, target, span })
}

/// Parse a `fairness_path` or bare identifier into a [`FairnessTarget`].
///
/// `fairness_path` exposes only `identifier` children; the brackets / dots
/// are anonymous tokens. We parse the raw textual form to recover
/// `Comp[*].action`, `Comp[id].action`, and `Comp[*].receives.Msg` shapes.
pub(super) fn parse_fairness_target(node: Node, cvt: &Cvt) -> surfacide_ast::substrate::FairnessTarget {
    use surfacide_ast::substrate::FairnessTarget;
    let raw = cvt.text(node).trim();
    let idents: Vec<Ident> = cvt
        .named_children_of(node, &["identifier"])
        .into_iter()
        .map(|n| ident_from_node(n, cvt))
        .collect();
    let span = cvt.span(node);

    // Detect `[*]` and `[id]` from raw text.
    let star_pos = raw.find("[*]");
    let bracket_pos = raw.find('[');
    let receives = raw.contains(".receives.");

    if let Some(star) = star_pos {
        // Comp[*].action  or Comp[*].receives.Msg
        let comp_text = raw[..star].trim_end();
        let comp = qualified_from_text(comp_text, &idents, span);
        if receives {
            // Last identifier is the message
            let message = idents.last().cloned().unwrap_or(Ident::new("<missing>", span));
            return FairnessTarget::ReceivesAllReplicas { component: comp, message };
        }
        let action = idents.last().cloned().unwrap_or(Ident::new("<missing>", span));
        return FairnessTarget::AllReplicas { component: comp, action };
    }
    if let Some(b) = bracket_pos {
        // Comp[id].action — collect `id` from the identifier between brackets.
        let comp_text = raw[..b].trim_end();
        let inside = &raw[b + 1..];
        let id_str = inside.split(']').next().unwrap_or("").trim();
        let id = Ident::new(id_str, span);
        let comp = qualified_from_text(comp_text, &idents, span);
        let action = idents.last().cloned().unwrap_or(Ident::new("<missing>", span));
        return FairnessTarget::SpecificReplica { component: comp, id, action };
    }
    // Plain dotted path / identifier.
    if idents.is_empty() {
        // `node.kind() == "identifier"` case.
        if node.kind() == "identifier" {
            return FairnessTarget::Path(QualifiedName::new(vec![ident_from_node(node, cvt)]));
        }
        return FairnessTarget::Path(QualifiedName::new(vec![Ident::new(raw, span)]));
    }
    FairnessTarget::Path(QualifiedName::new(idents))
}

fn qualified_from_text(text: &str, idents: &[Ident], span: Span) -> QualifiedName {
    // Rebuild a QualifiedName from the dot-separated `text`, preferring
    // matching idents from the supplied list to preserve their spans.
    let segs: Vec<Ident> = text
        .split('.')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            idents
                .iter()
                .find(|i| i.name == s)
                .cloned()
                .unwrap_or_else(|| Ident::new(s, span))
        })
        .collect();
    if segs.is_empty() {
        QualifiedName::new(vec![Ident::new(text, span)])
    } else {
        QualifiedName::new(segs)
    }
}

fn convert_observable_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<ObservableDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let params = if let Some(plist) = cvt.first_child(node, "param_list") {
        convert_param_list(plist, cvt).into_iter().map(Into::into).collect()
    } else {
        Vec::new()
    };
    let return_ty = node
        .child_by_field_name("return_type")
        .map(|n| convert_type_expr(n, cvt))?;
    let body_node = node.child_by_field_name("body")?;
    let body = convert_expr::convert_expr(body_node, cvt);
    Some(ObservableDecl {
        name,
        for_actor: None,
        params,
        return_ty,
        body,
        doc,
        span,
    })
}

fn convert_actor_observable_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<ObservableDecl> {
    let span = cvt.span(node);
    let actor_var = node
        .child_by_field_name("actor_var")
        .map(|n| ident_from_node(n, cvt))?;
    let actor_ty = node
        .child_by_field_name("actor_type")
        .map(|n| ident_from_node(n, cvt))?;
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let params = if let Some(plist) = cvt.first_child(node, "param_list") {
        convert_param_list(plist, cvt).into_iter().map(Into::into).collect()
    } else {
        Vec::new()
    };
    let return_ty = node
        .child_by_field_name("return_type")
        .map(|n| convert_type_expr(n, cvt))?;
    let body_node = node.child_by_field_name("body")?;
    let body = convert_expr::convert_expr(body_node, cvt);
    let for_actor = Some(ActorBinder {
        name: actor_var.clone(),
        actor_ty,
        span: actor_var.span,
    });
    Some(ObservableDecl {
        name,
        for_actor,
        params,
        return_ty,
        body,
        doc,
        span,
    })
}

fn convert_history_predicate_decl(
    node: Node,
    cvt: &mut Cvt,
    doc: Option<String>,
) -> Option<HistoryPredicateDecl> {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt))?;
    let params = if let Some(plist) = cvt.first_child(node, "param_list") {
        convert_param_list(plist, cvt).into_iter().map(Into::into).collect()
    } else {
        Vec::new()
    };
    let body_node = node.child_by_field_name("body")?;
    let body = convert_expr::convert_expr(body_node, cvt);
    Some(HistoryPredicateDecl { name, params, body, doc, span })
}
