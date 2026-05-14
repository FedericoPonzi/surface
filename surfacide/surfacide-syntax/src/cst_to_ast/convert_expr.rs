//! Convert CST expression nodes to AST.
//!
//! The expression grammar is the bulk of Surface; this module is the
//! largest in the converter.

use super::{ident_from_node, convert_qualified_name, Cvt};
use surfacide_ast::*;
use surfacide_ast::expr::*;
use tree_sitter::Node;

/// Convert any expression-position CST node to an AST [`Expr`].
///
/// Deliberately tolerant: unknown kinds become a synthetic placeholder
/// with a diagnostic so downstream passes can still navigate.
pub fn convert_expr(node: Node, cvt: &mut Cvt) -> Expr {
    let span = cvt.span(node);
    let kind = convert_expr_kind(node, cvt);
    Expr { kind: Box::new(kind), span }
}

fn placeholder(node: Node, cvt: &Cvt) -> ExprKind {
    ExprKind::LitString(format!("<unhandled:{}>", node.kind()))
}

fn first_named_child<'b>(n: Node<'b>, kind: &str) -> Option<Node<'b>> {
    let mut cursor = n.walk();
    for c in n.named_children(&mut cursor) {
        if c.kind() == kind {
            return Some(c);
        }
    }
    None
}

fn convert_expr_kind(node: Node, cvt: &mut Cvt) -> ExprKind {
    match node.kind() {
        // Literals
        "number" => {
            let text = cvt.text(node);
            text.parse::<u64>()
                .map(ExprKind::LitNat)
                .unwrap_or_else(|_| {
                    text.parse::<i64>()
                        .map(ExprKind::LitInt)
                        .unwrap_or_else(|_| ExprKind::LitString(text.to_string()))
                })
        }
        "bool_lit" => ExprKind::LitBool(cvt.text(node) == "true"),
        "string" => {
            let raw = cvt.text(node);
            let s = raw
                .strip_prefix("\"\"\"")
                .and_then(|r| r.strip_suffix("\"\"\""))
                .or_else(|| raw.strip_prefix('"').and_then(|r| r.strip_suffix('"')))
                .unwrap_or(raw);
            ExprKind::LitString(s.to_string())
        }
        "none_lit" => ExprKind::LitNone,

        // Atoms / wrappers
        "identifier" => ExprKind::Ident(ident_from_node(node, cvt)),
        "identifier_expr" | "parenthesized" => cvt
            .all_named_children(node)
            .first()
            .map(|c| convert_expr_kind(*c, cvt))
            .unwrap_or_else(|| placeholder(node, cvt)),
        "param_ref" => {
            if let Some(id) = first_named_child(node, "identifier") {
                ExprKind::Path(PathExpr {
                    head: Ident::new("param", cvt.span(node)),
                    accessors: vec![PathAccessor::Field(ident_from_node(id, cvt))],
                })
            } else {
                placeholder(node, cvt)
            }
        }

        // Qualified / path-like
        "qualified_name" => {
            let qn = convert_qualified_name(node, cvt);
            if qn.is_simple() {
                ExprKind::Ident(qn.segments.into_iter().next().unwrap())
            } else {
                let mut iter = qn.segments.into_iter();
                let head = iter.next().unwrap();
                let accessors: Vec<PathAccessor> = iter.map(PathAccessor::Field).collect();
                ExprKind::Path(PathExpr { head, accessors })
            }
        }
        "field_access" | "dotted_field" | "dotted_index" | "indexed_path" | "realization_path"
        | "channel_path" | "fairness_path" | "index_expr" => convert_path_like(node, cvt),

        // Tuple / record / set / seq / map
        "tuple_expr" => {
            let elts: Vec<Expr> = cvt
                .all_named_children(node)
                .into_iter()
                .map(|c| convert_expr(c, cvt))
                .collect();
            ExprKind::Tuple(elts)
        }
        "record_expr" => {
            let fields: Vec<RecordFieldInit> = cvt
                .named_children_of(node, &["record_field"])
                .into_iter()
                .map(|f| {
                    let fspan = cvt.span(f);
                    let name = f
                        .child_by_field_name("name")
                        .map(|n| Ident::new(cvt.text(n), cvt.span(n)))
                        .unwrap_or_else(|| Ident::new("<missing>", fspan));
                    let value = f
                        .child_by_field_name("value")
                        .map(|n| convert_expr(n, cvt))
                        .unwrap_or(Expr {
                            kind: Box::new(ExprKind::LitNone),
                            span: fspan,
                        });
                    RecordFieldInit { name, value, span: fspan }
                })
                .collect();
            ExprKind::Record(fields)
        }
        "set_expr" => {
            let elts: Vec<Expr> = cvt
                .all_named_children(node)
                .into_iter()
                .map(|c| convert_expr(c, cvt))
                .collect();
            ExprKind::SetLit(elts)
        }
        "seq_literal" => {
            let elts: Vec<Expr> = cvt
                .all_named_children(node)
                .into_iter()
                .map(|c| convert_expr(c, cvt))
                .collect();
            ExprKind::SeqLit(elts)
        }
        "map_literal" => {
            let entries: Vec<(Expr, Expr)> = cvt
                .named_children_of(node, &["map_literal_entry"])
                .into_iter()
                .map(|e| {
                    let kv: Vec<Node> = cvt.all_named_children(e);
                    let k = kv.first().map(|n| convert_expr(*n, cvt)).unwrap_or(Expr {
                        kind: Box::new(ExprKind::LitNone),
                        span: cvt.span(e),
                    });
                    let v = kv.get(1).map(|n| convert_expr(*n, cvt)).unwrap_or(Expr {
                        kind: Box::new(ExprKind::LitNone),
                        span: cvt.span(e),
                    });
                    (k, v)
                })
                .collect();
            ExprKind::MapLit(entries)
        }
        "empty_brace_expr" => ExprKind::SetLit(Vec::new()),

        // Operators
        "binary_expr" | "binary_expr_no_in" => convert_binary(node, cvt),
        "neg_expr" | "neg_expr_no_in" => {
            let inner = cvt
                .all_named_children(node)
                .into_iter()
                .next()
                .map(|c| convert_expr(c, cvt))
                .unwrap_or(Expr {
                    kind: Box::new(ExprKind::LitNone),
                    span: cvt.span(node),
                });
            ExprKind::UnaryOp(UnaryOp::Neg, inner)
        }
        "not_expr" | "not_expr_no_in" => {
            let inner = cvt
                .all_named_children(node)
                .into_iter()
                .next()
                .map(|c| convert_expr(c, cvt))
                .unwrap_or(Expr {
                    kind: Box::new(ExprKind::LitNone),
                    span: cvt.span(node),
                });
            ExprKind::UnaryOp(UnaryOp::Not, inner)
        }
        "cardinality_expr" => {
            let inner = cvt
                .all_named_children(node)
                .into_iter()
                .next()
                .map(|c| convert_expr(c, cvt))
                .unwrap_or(Expr {
                    kind: Box::new(ExprKind::LitNone),
                    span: cvt.span(node),
                });
            ExprKind::Cardinality(inner)
        }

        // Quantifiers
        "forall_expr" | "exists_expr" => convert_quantifier(node, cvt),

        // Comprehensions
        "comprehension" => convert_comprehension(node, cvt),

        // Choose / aggregate
        "choose_expr" | "choose_expr_no_in" => convert_choose(node, cvt),
        "aggregate_expr" => convert_aggregate(node, cvt),

        // Control flow
        "if_expr" => convert_if_expr(node, cvt),
        "match_expr" => convert_match_expr(node, cvt),
        "if_let_expr" => convert_if_let_expr(node, cvt),
        "let_expr" => convert_let_expr(node, cvt),

        // Function-like calls
        "call_expr" | "slot_call" => convert_call_expr(node, cvt),
        "some_call" => {
            let inner = cvt
                .all_named_children(node)
                .into_iter()
                .next()
                .map(|c| convert_expr(c, cvt))
                .unwrap_or(Expr {
                    kind: Box::new(ExprKind::LitNone),
                    span: cvt.span(node),
                });
            ExprKind::Some_(inner)
        }

        _ => placeholder(node, cvt),
    }
}

fn convert_path_like(node: Node, cvt: &mut Cvt) -> ExprKind {
    let mut accessors: Vec<PathAccessor> = Vec::new();
    let mut current = node;
    let head: Ident;
    loop {
        match current.kind() {
            "field_access" | "dotted_field" => {
                // Grammar: `field_access` has `object` field + a
                // dotted_field/dotted_index LEAF token child whose text
                // is ".name" / ".0" (self-review fold #2).
                let base = current
                    .child_by_field_name("object")
                    .or_else(|| current.child_by_field_name("base"));
                let mut cursor = current.walk();
                for child in current.named_children(&mut cursor) {
                    match child.kind() {
                        "dotted_field" => {
                            let text = cvt.text(child);
                            let name = text.strip_prefix('.').unwrap_or(text);
                            accessors.insert(
                                0,
                                PathAccessor::Field(Ident::new(name, cvt.span(child))),
                            );
                        }
                        "dotted_index" => {
                            let text = cvt.text(child);
                            let n: u32 = text
                                .strip_prefix('.')
                                .unwrap_or(text)
                                .parse()
                                .unwrap_or(0);
                            accessors.insert(0, PathAccessor::TupleIndex(n));
                        }
                        _ => {}
                    }
                }
                if let Some(b) = base {
                    current = b;
                    continue;
                }
                head = Ident::new("<missing>", cvt.span(current));
                break;
            }
            "dotted_index" => {
                // Dead arm in practice — dotted_index only appears as a
                // child of field_access, handled above. Kept defensively.
                let text = cvt.text(current);
                let n: u32 = text.strip_prefix('.').unwrap_or(text).parse().unwrap_or(0);
                accessors.insert(0, PathAccessor::TupleIndex(n));
                head = Ident::new("<missing>", cvt.span(current));
                break;
            }
            "index_expr" | "indexed_path" => {
                // Grammar field is `object`, not `base` (R9 finding via debug).
                let base = current
                    .child_by_field_name("object")
                    .or_else(|| current.child_by_field_name("base"));
                let idx = current.child_by_field_name("index");
                if let Some(i) = idx {
                    let inner_expr = convert_expr(i, cvt);
                    accessors.insert(0, PathAccessor::Index(inner_expr));
                }
                if let Some(b) = base {
                    current = b;
                    continue;
                }
                head = Ident::new("<missing>", cvt.span(current));
                break;
            }
            "identifier" => {
                head = ident_from_node(current, cvt);
                break;
            }
            "qualified_name" => {
                let qn = convert_qualified_name(current, cvt);
                let mut segs = qn.segments.into_iter();
                head = segs.next().unwrap_or_else(|| Ident::new("<missing>", cvt.span(current)));
                for s in segs {
                    accessors.insert(0, PathAccessor::Field(s));
                }
                break;
            }
            _ => {
                if let Some(c) = cvt.all_named_children(current).into_iter().next() {
                    current = c;
                    continue;
                }
                head = Ident::new("<missing>", cvt.span(current));
                break;
            }
        }
    }
    ExprKind::Path(PathExpr { head, accessors })
}

fn convert_binary(node: Node, cvt: &mut Cvt) -> ExprKind {
    let kids = cvt.all_named_children(node);
    if kids.len() < 2 {
        return ExprKind::LitNone;
    }
    let left = convert_expr(kids[0], cvt);
    let right = convert_expr(kids[kids.len() - 1], cvt);
    let op_text = cvt
        .source
        .get(kids[0].end_byte()..kids[kids.len() - 1].start_byte())
        .unwrap_or("")
        .trim();
    let op = match op_text {
        "==" => BinOp::Eq,
        "!=" => BinOp::NotEq,
        "<" => BinOp::Lt,
        "<=" => BinOp::LtEq,
        ">" => BinOp::Gt,
        ">=" => BinOp::GtEq,
        "+" => BinOp::Add,
        "-" => BinOp::Sub,
        "*" => BinOp::Mul,
        "/" => BinOp::Div,
        "&&" => BinOp::And,
        "||" => BinOp::Or,
        "=>" => BinOp::Implies,
        "in" => BinOp::In,
        "not in" => BinOp::NotIn,
        "union" => BinOp::Union,
        "intersect" => BinOp::Intersect,
        "diff" => BinOp::Diff,
        ":+" => BinOp::SeqSnoc,
        "subset" => BinOp::Subset,
        "cross" => return ExprKind::Cross(left, right),
        "is" => {
            if let ExprKind::Ident(name) = *right.kind {
                return ExprKind::IsTest(left, name);
            }
            return ExprKind::LitBool(false);
        }
        other => {
            cvt.internal_error(node, format!("unknown binary op `{}`", other));
            return ExprKind::LitNone;
        }
    };
    ExprKind::BinOp(op, left, right)
}

fn convert_quantifier(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    // Grammar: forall_expr/exists_expr have one or more `binder` children
    // plus a `body` field. Previously we read `var`/`domain` directly from
    // the quantifier node — those fields don't exist (self-review-2 #3).
    let binder = cvt
        .all_named_children(node)
        .into_iter()
        .find(|c| c.kind() == "binder");
    let (name, domain) = if let Some(b) = binder {
        let n = b
            .child_by_field_name("name")
            .map(|n| ident_from_node(n, cvt))
            .unwrap_or_else(|| Ident::new("<missing>", span));
        let d = b
            .child_by_field_name("source")
            .map(|n| convert_expr(n, cvt))
            .unwrap_or(Expr {
                kind: Box::new(ExprKind::LitNone),
                span,
            });
        (n, d)
    } else {
        (
            Ident::new("<missing>", span),
            Expr { kind: Box::new(ExprKind::LitNone), span },
        )
    };
    let body = node
        .child_by_field_name("body")
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitBool(true)),
            span,
        });
    let binding = Binding { name: name.clone(), domain, span: name.span };
    if node.kind() == "forall_expr" {
        ExprKind::Forall(binding, body)
    } else {
        ExprKind::Exists(binding, body)
    }
}

fn convert_comprehension(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    let map_arrow = first_named_child(node, "comp_map_arrow").is_some();
    let binders: Vec<ComprehensionBinder> = cvt
        .named_children_of(node, &["comp_binder"])
        .into_iter()
        .map(|b| convert_comp_binder(b, cvt))
        .collect();
    let kids = cvt.all_named_children(node);
    let body_kids: Vec<Node> = kids
        .iter()
        .copied()
        .filter(|c| !matches!(c.kind(), "comp_binder" | "comp_map_arrow"))
        .collect();
    if map_arrow {
        let key = body_kids
            .first()
            .map(|n| convert_expr(*n, cvt))
            .unwrap_or(Expr {
                kind: Box::new(ExprKind::LitNone),
                span,
            });
        let value = body_kids
            .get(1)
            .map(|n| convert_expr(*n, cvt))
            .unwrap_or(Expr {
                kind: Box::new(ExprKind::LitNone),
                span,
            });
        let predicate = body_kids.get(2).map(|n| convert_expr(*n, cvt));
        ExprKind::MapComprehension { binders, predicate, key, value }
    } else {
        let body = body_kids
            .first()
            .map(|n| convert_expr(*n, cvt))
            .unwrap_or(Expr {
                kind: Box::new(ExprKind::LitNone),
                span,
            });
        let predicate = body_kids.get(1).map(|n| convert_expr(*n, cvt));
        ExprKind::SetComprehension { binders, predicate, body }
    }
}

fn convert_comp_binder(node: Node, cvt: &mut Cvt) -> ComprehensionBinder {
    let span = cvt.span(node);
    // Grammar field is `name` (with type identifier|tuple_pattern) and
    // `source` (the domain). Earlier we read `pattern`/`domain` — now
    // matched against node-types.json (self-review-2 #3).
    let pattern_node = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("pattern"));
    let pattern = if let Some(pn) = pattern_node {
        match pn.kind() {
            "tuple_pattern" => {
                let names: Vec<Ident> = cvt
                    .named_children_of(pn, &["identifier"])
                    .into_iter()
                    .map(|n| ident_from_node(n, cvt))
                    .collect();
                BinderPattern::Tuple(names)
            }
            "identifier" => {
                let id = ident_from_node(pn, cvt);
                let ty = node
                    .child_by_field_name("type")
                    .map(|n| super::convert_type_expr(n, cvt));
                BinderPattern::Name(id, ty)
            }
            _ => BinderPattern::Name(Ident::new("<missing>", span), None),
        }
    } else {
        BinderPattern::Name(Ident::new("<missing>", span), None)
    };
    let domain = node
        .child_by_field_name("source")
        .or_else(|| node.child_by_field_name("domain"))
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitNone),
            span,
        });
    ComprehensionBinder { pattern, domain, span }
}

fn convert_choose(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("var")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let predicate = node
        .child_by_field_name("predicate")
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitBool(true)),
            span,
        });
    // Grammar field is `source`, not `domain` (self-review-2 finding).
    if let Some(dom) = node.child_by_field_name("source").or_else(|| node.child_by_field_name("domain")) {
        ExprKind::ChooseBounded {
            name,
            domain: convert_expr(dom, cvt),
            predicate,
        }
    } else if let Some(ty) = node.child_by_field_name("type") {
        ExprKind::ChooseTyped {
            name,
            ty: super::convert_type_expr(ty, cvt),
            predicate,
        }
    } else {
        ExprKind::ChooseBounded {
            name,
            domain: Expr { kind: Box::new(ExprKind::LitNone), span },
            predicate,
        }
    }
}

fn convert_aggregate(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    // Grammar shape: `aggregate <body> [over <scope>] using <aggregator> [else <else>]`.
    // The AST type AggregateExpr predates this grammar and still carries
    // `component`/`binder`; for now we leave those placeholder and map
    // `body`→expr, `else`→fallback (self-review-2 #3).
    let expr = node
        .child_by_field_name("body")
        .or_else(|| node.child_by_field_name("expr"))
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitNone),
            span,
        });
    let scope = node.child_by_field_name("scope").map(|n| convert_expr(n, cvt));
    let aggregator = node
        .child_by_field_name("aggregator")
        .map(|n| convert_aggregator(n, cvt))
        .unwrap_or(AggregatorKind::Exists);
    let fallback = node
        .child_by_field_name("else")
        .or_else(|| node.child_by_field_name("fallback"))
        .map(|n| convert_expr(n, cvt));
    let component = node
        .child_by_field_name("component")
        .map(|n| convert_qualified_name(n, cvt))
        .unwrap_or_else(|| QualifiedName::new(vec![Ident::new("<aggregate>", span)]));
    let binder = node
        .child_by_field_name("binder")
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("_", span));
    ExprKind::Aggregate(Box::new(AggregateExpr {
        component,
        binder,
        expr,
        scope,
        aggregator,
        fallback,
        span,
    }))
}

fn convert_aggregator(node: Node, cvt: &mut Cvt) -> AggregatorKind {
    if node.kind() == "concat_seq_aggregator" {
        let order_by = node
            .child_by_field_name("order_by")
            .map(|n| ident_from_node(n, cvt))
            .unwrap_or_else(|| Ident::new("<missing>", cvt.span(node)));
        return AggregatorKind::ConcatSeq { order_by };
    }
    match cvt.text(node) {
        "exists" => AggregatorKind::Exists,
        "forall" => AggregatorKind::Forall,
        "sum" => AggregatorKind::Sum,
        "max" => AggregatorKind::Max,
        "min" => AggregatorKind::Min,
        "union_set" => AggregatorKind::UnionSet,
        _ => AggregatorKind::Exists,
    }
}

fn convert_if_expr(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    let cond = node.child_by_field_name("cond").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitBool(false)),
        span,
    });
    let then_branch = node.child_by_field_name("then").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitNone),
        span,
    });
    let else_branch = node.child_by_field_name("else").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitNone),
        span,
    });
    ExprKind::IfThenElse { cond, then_branch, else_branch }
}

fn convert_match_expr(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    let scrutinee = node.child_by_field_name("scrutinee").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitNone),
        span,
    });
    let arms: Vec<MatchArm> = cvt
        .named_children_of(node, &["match_arm"])
        .into_iter()
        .map(|a| convert_match_arm(a, cvt))
        .collect();
    ExprKind::Match { scrutinee, arms }
}

fn convert_match_arm(node: Node, cvt: &mut Cvt) -> MatchArm {
    let span = cvt.span(node);
    let pat_node = node.child_by_field_name("pattern");
    let pattern = match pat_node {
        Some(p) => match p.kind() {
            "match_pattern" => {
                let inner = cvt.all_named_children(p);
                inner
                    .first()
                    .map(|n| match n.kind() {
                        "some_call" => {
                            let id = cvt
                                .all_named_children(*n)
                                .into_iter()
                                .next()
                                .map(|c| ident_from_node(c, cvt))
                                .unwrap_or_else(|| Ident::new("<missing>", cvt.span(*n)));
                            MatchPattern::Some_(id)
                        }
                        "none_lit" => MatchPattern::None_,
                        "wildcard" => MatchPattern::Wildcard,
                        _ => MatchPattern::Wildcard,
                    })
                    .unwrap_or(MatchPattern::Wildcard)
            }
            "wildcard" => MatchPattern::Wildcard,
            "none_lit" => MatchPattern::None_,
            "some_call" => {
                let id = cvt
                    .all_named_children(p)
                    .into_iter()
                    .next()
                    .map(|c| ident_from_node(c, cvt))
                    .unwrap_or_else(|| Ident::new("<missing>", cvt.span(p)));
                MatchPattern::Some_(id)
            }
            _ => MatchPattern::Wildcard,
        },
        None => MatchPattern::Wildcard,
    };
    let body = node
        .child_by_field_name("body")
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitNone),
            span,
        });
    MatchArm { pattern, body, span }
}

fn convert_if_let_expr(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    // Grammar fields are `binding` + `value` (self-review-2 #3).
    let name = node
        .child_by_field_name("binding")
        .or_else(|| node.child_by_field_name("var"))
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let source = node
        .child_by_field_name("value")
        .or_else(|| node.child_by_field_name("source"))
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitNone),
            span,
        });
    let then_branch = node.child_by_field_name("then").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitNone),
        span,
    });
    let else_branch = node.child_by_field_name("else").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitNone),
        span,
    });
    ExprKind::IfLetSome { name, source, then_branch, else_branch }
}

fn convert_let_expr(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    let name = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("var"))
        .map(|n| ident_from_node(n, cvt))
        .unwrap_or_else(|| Ident::new("<missing>", span));
    let value = node.child_by_field_name("value").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitNone),
        span,
    });
    let body = node.child_by_field_name("body").map(|n| convert_expr(n, cvt)).unwrap_or(Expr {
        kind: Box::new(ExprKind::LitNone),
        span,
    });
    ExprKind::Let { name, value, body }
}

fn convert_call_expr(node: Node, cvt: &mut Cvt) -> ExprKind {
    let span = cvt.span(node);
    let callee = node
        .child_by_field_name("callee")
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitNone),
            span,
        });
    let args: Vec<CallArg> = if let Some(args_node) = first_named_child(node, "call_args") {
        cvt.all_named_children(args_node)
            .into_iter()
            .map(|a| convert_call_arg(a, cvt))
            .collect()
    } else {
        cvt.named_children_of(node, &["call_arg", "named_arg"])
            .into_iter()
            .map(|a| convert_call_arg(a, cvt))
            .collect()
    };

    // Specialise event-log helpers and state_at so downstream sees typed
    // variants.
    if let ExprKind::Ident(id) = &*callee.kind {
        let name = id.name.as_str();
        let arg_exprs: Vec<Expr> = args.iter().map(|a| a.value.clone()).collect();
        match (name, arg_exprs.len()) {
            ("events_before", 1) => return ExprKind::EventsBefore(arg_exprs.into_iter().next().unwrap()),
            ("events_after", 1) => return ExprKind::EventsAfter(arg_exprs.into_iter().next().unwrap()),
            ("between", 2) => {
                let mut i = arg_exprs.into_iter();
                return ExprKind::Between(i.next().unwrap(), i.next().unwrap());
            }
            ("first", 1) => return ExprKind::FirstUnbounded(arg_exprs.into_iter().next().unwrap()),
            ("first", 2) => {
                let mut i = arg_exprs.into_iter();
                return ExprKind::FirstBounded(i.next().unwrap(), i.next().unwrap());
            }
            ("last", 1) => return ExprKind::LastUnbounded(arg_exprs.into_iter().next().unwrap()),
            ("last", 2) => {
                let mut i = arg_exprs.into_iter();
                return ExprKind::LastBounded(i.next().unwrap(), i.next().unwrap());
            }
            ("count", 1) => return ExprKind::CountUnbounded(arg_exprs.into_iter().next().unwrap()),
            ("count", 2) => {
                let mut i = arg_exprs.into_iter();
                return ExprKind::CountBounded(i.next().unwrap(), i.next().unwrap());
            }
            ("state_at", 1) => return ExprKind::StateAt(arg_exprs.into_iter().next().unwrap()),
            _ => {}
        }
    }

    ExprKind::Call { callee, args }
}

pub(super) fn convert_call_arg(node: Node, cvt: &mut Cvt) -> CallArg {
    let span = cvt.span(node);
    let name = node.child_by_field_name("name").map(|n| ident_from_node(n, cvt));
    let value_node = node.child_by_field_name("value").or_else(|| {
        cvt.all_named_children(node).into_iter().next()
    });
    let value = value_node
        .map(|n| convert_expr(n, cvt))
        .unwrap_or(Expr {
            kind: Box::new(ExprKind::LitNone),
            span,
        });
    CallArg { name, value, span }
}
