/**
 * Tree-sitter grammar for the Surface specification language (v0.10.1).
 *
 * Whitespace-insensitive, brace-delimited; statement separators inside a block
 * are newlines or `;` (both optional — the grammar relies on each statement
 * starting with a token that cannot appear as the continuation of the previous
 * one).
 */

const PREC = {
  let_expr: 0,
  if_expr: 0,
  match_expr: 0,
  quantifier: 1,
  pipe: 1,
  or: 2,
  and: 3,
  not: 4,
  compare: 5,
  set_op: 6,
  add: 7,
  mul: 8,
  unary: 9,
  postfix: 10,
  application: 11,
  field: 12,
  cardinality: 13,
  type_union: 1,
  type_arrow: 2,
};

function commaSep(rule) {
  return optional(commaSep1(rule));
}

function commaSep1(rule) {
  return seq(rule, repeat(seq(',', rule)));
}

function sepBy(sep, rule) {
  return optional(sepBy1(sep, rule));
}

function sepBy1(sep, rule) {
  return seq(rule, repeat(seq(sep, rule)));
}

module.exports = grammar({
  name: 'surface',

  word: $ => $.identifier,

  extras: $ => [
    /\s/,
    $.line_comment,
    $.block_comment,
  ],

  conflicts: $ => [
    // Bare identifier in expression context vs the start of an action call /
    // qualified path (used by realizes clauses, scenarios, etc.).
    [$.identifier_expr, $._call_target],

    // Type vs expression record/set ambiguity inside a few positions.
    [$.record_type, $.record_expr],
    [$.set_expr, $.record_expr],
    [$.set_expr, $.comprehension],
    [$.record_expr, $.comprehension],
    [$.record_field, $.identifier_expr],
    [$.record_field, $._primary_expr],
    [$.record_field, $.comp_map_arrow],
    [$.tuple_expr_key, $.tuple_expr],
    [$.empty_brace_expr, $.record_type],

    // Ambiguity between a bare identifier as a type and as an expression
    // when both are reachable from a slot value or scenario predicate.
    [$.simple_type, $.identifier_expr],
    [$.qualified_type, $.qualified_expr],
    [$.simple_type, $.qualified_type],
    [$.then_block],
    [$.if_effect],
    [$.else_clause],
    [$.map_assign_effect, $._lvalue],
    [$.comprehension, $.map_literal],
    [$.map_literal_entry, $.comp_map_arrow],
    [$.set_expr, $.map_literal],
    [$._primary_expr, $.map_literal_entry],

    // qualified_expr vs ack_path (similar shape).
    [$.qualified_expr, $._ack_lhs],
  ],

  rules: {
    source_file: $ => seq(
      $.module_header,
      repeat($._top_level),
    ),

    // ── Comments & strings ─────────────────────────────────────────────────

    line_comment: $ => token(seq('--', /[^\n]*/)),

    block_comment: $ => token(seq(
      '{-',
      /[^-]*-+([^-}][^-]*-+)*/,
      '}',
    )),

    string: $ => choice(
      $._triple_string,
      $._simple_string,
    ),

    _simple_string: $ => token(seq(
      '"',
      repeat(choice(/[^"\\\n]/, /\\./)),
      '"',
    )),

    _triple_string: $ => token(seq(
      '"""',
      /([^"]|"[^"]|""[^"])*/,
      '"""',
    )),

    // ── Module header & imports ────────────────────────────────────────────

    module_header: $ => seq(
      'module',
      field('name', $.qualified_name),
      optional(field('visibility', 'private')),
    ),

    use_decl: $ => seq(
      'use',
      field('module', $.qualified_name),
      optional(seq('.', '{', commaSep1(field('imported', $.identifier)), '}')),
    ),

    // ── Top-level declarations ─────────────────────────────────────────────

    _top_level: $ => choice(
      $.use_decl,
      $.doc_string,
      $.type_decl,
      $.actor_decl,
      $.event_decl,
      $.const_decl,
      $.extern_decl,
      $.observable_decl,
      $.actor_observable_decl,
      $.history_predicate_decl,
      $.attacker_decl,
      $.scenario_decl,
      $.property_decl,
      $.surface_block,
      $.substrate_block,
      $.partial_substrate_block,
      $.compose_block,
      $.tla_block,
    ),

    doc_string: $ => $.string,

    type_decl: $ => seq(
      'type',
      field('name', $.identifier),
      '=',
      field('value', $.type_expr),
    ),

    actor_decl: $ => seq(
      'actor',
      field('name', $.identifier),
      optional(seq('extends', field('parent', $.identifier))),
    ),

    event_decl: $ => seq(
      'event',
      field('name', $.identifier),
      $.param_list,
    ),

    const_decl: $ => seq(
      'const',
      field('name', $.identifier),
      ':',
      field('type', $.type_expr),
    ),

    extern_decl: $ => seq(
      'extern',
      field('name', $.identifier),
      ':',
      field('type', $.type_expr),
    ),

    observable_decl: $ => seq(
      'observable',
      field('name', $.identifier),
      $.param_list,
      ':',
      field('return_type', $.type_expr),
      '=',
      field('body', $._expression),
    ),

    actor_observable_decl: $ => seq(
      'observable',
      'for',
      field('actor_var', $.identifier),
      ':',
      field('actor_type', $.identifier),
      field('name', $.identifier),
      $.param_list,
      ':',
      field('return_type', $.type_expr),
      '=',
      field('body', $._expression),
    ),

    history_predicate_decl: $ => seq(
      'history_predicate',
      field('name', $.identifier),
      $.param_list,
      '{',
      field('body', $._expression),
      '}',
    ),

    property_decl: $ => seq(
      'property',
      field('name', $.identifier),
      '{',
      field('body', $._property_body),
      '}',
    ),

    _property_body: $ => choice(
      seq('always', $._expression),
      seq('eventually', $._expression),
      $._expression,
    ),

    // ── Parameter lists & types ────────────────────────────────────────────

    param_list: $ => seq(
      '(',
      commaSep($.param),
      ')',
    ),

    param: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $.type_expr),
    ),

    type_expr: $ => choice(
      $._non_union_type,
      $.union_type,
    ),

    union_type: $ => prec.left(PREC.type_union, seq(
      $._non_union_type,
      repeat1(seq('|', $._non_union_type)),
    )),

    _non_union_type: $ => choice(
      $.simple_type,
      $.qualified_type,
      $.generic_type,
      $.tuple_type,
      $.record_type,
      $.enum_type,
    ),

    simple_type: $ => $.identifier,

    qualified_type: $ => prec.right(seq(
      $.identifier,
      repeat1(seq('.', $.identifier)),
    )),

    generic_type: $ => seq(
      field('base', $.identifier),
      '[',
      choice(
        seq(field('key', $.type_expr), '->', field('value', $.type_expr)),
        commaSep1($.type_expr),
      ),
      ']',
    ),

    tuple_type: $ => seq(
      '(',
      $.type_expr,
      ',',
      commaSep1($.type_expr),
      ')',
    ),

    record_type: $ => seq(
      '{',
      commaSep1($.record_type_field),
      optional(','),
      '}',
    ),

    record_type_field: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $.type_expr),
    ),

    enum_type: $ => seq(
      'enum',
      '{',
      commaSep1(field('variant', $.identifier)),
      optional(','),
      '}',
    ),

    // ── Surface block ──────────────────────────────────────────────────────

    surface_block: $ => seq(
      'surface',
      '{',
      repeat($._surface_item),
      '}',
    ),

    _surface_item: $ => choice(
      $.doc_string,
      $.defaults_block,
      $.state_block,
      $.init_block,
      $.fairness_decl,
      $.observable_decl,
      $.actor_observable_decl,
      $.property_decl,
      $.action_decl,
      $.internal_action_decl,
    ),

    defaults_block: $ => seq(
      'defaults',
      '{',
      repeat($.slot_assign),
      '}',
    ),

    state_block: $ => seq(
      'state',
      '{',
      repeat(choice($.state_field, $.doc_string)),
      '}',
    ),

    state_field: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $.type_expr),
      repeat($._state_modifier),
    ),

    _state_modifier: $ => choice(
      $.retention_modifier,
      $.private_modifier,
      $.derived_modifier,
    ),

    retention_modifier: $ => seq(
      'retention',
      ':',
      field('value', $.slot_value),
    ),

    private_modifier: $ => 'private',

    derived_modifier: $ => seq(
      'derived',
      optional(seq('shape', ':', field('shape', $.identifier))),
      optional(seq('of', ':', field('of', $.type_expr))),
    ),

    init_block: $ => seq(
      'init',
      '{',
      repeat($._effect),
      '}',
    ),

    fairness_decl: $ => seq(
      'fairness',
      field('kind', choice('weak', 'strong')),
      field('target', $._fairness_target),
    ),

    _fairness_target: $ => choice(
      $.identifier,
      $.fairness_path,
    ),

    fairness_path: $ => seq(
      $.identifier,
      repeat1(choice(
        seq('.', $.identifier),
        seq('[', choice($.identifier, '*'), ']'),
      )),
    ),

    // ── Action declarations ────────────────────────────────────────────────

    action_decl: $ => seq(
      'action',
      field('name', $.identifier),
      $.param_list,
      optional(seq('->', field('return_type', $.type_expr))),
      repeat($._action_clause),
    ),

    internal_action_decl: $ => seq(
      'internal_action',
      field('name', $.identifier),
      $.param_list,
      optional(seq('->', field('return_type', $.type_expr))),
      repeat($._action_clause),
    ),

    _action_clause: $ => choice(
      $.by_clause,
      $.when_clause,
      $.raises_block,
      $.slot_assign,
      $.then_block,
    ),

    by_clause: $ => seq(
      'by',
      field('var', $.identifier),
      ':',
      field('type', $.identifier),
    ),

    when_clause: $ => seq(
      'when',
      field('guard', $._expression),
    ),

    raises_block: $ => seq(
      'raises',
      '{',
      commaSep($.raises_entry),
      optional(','),
      '}',
    ),

    raises_entry: $ => seq(
      field('name', $.identifier),
      'when',
      field('guard', $._expression),
    ),

    slot_assign: $ => seq(
      field('slot', $._slot_name),
      ':',
      field('value', $.slot_value),
    ),

    _slot_name: $ => choice(
      'idempotency',
      'auth_channel',
      'retention',
      'rate_limit',
      'observability',
      'availability',
      'freshness',
    ),

    slot_value: $ => choice(
      $.slot_waiver,
      $.slot_set,
      $.slot_call,
    ),

    slot_waiver: $ => seq('waived', ':', field('reason', $.string)),

    slot_set: $ => seq(
      '{',
      commaSep1(field('channel', $.identifier)),
      optional(','),
      '}',
    ),

    slot_call: $ => prec.right(seq(
      field('name', $.identifier),
      optional(field('args', $.call_args)),
      optional(field('by', $.idempotency_by)),
    )),

    idempotency_by: $ => seq(
      'by',
      '(',
      commaSep1(field('arg', $.identifier)),
      ')',
    ),

    call_args: $ => seq(
      '(',
      commaSep($.call_arg),
      ')',
    ),

    call_arg: $ => choice(
      $.named_arg,
      $._expression,
    ),

    named_arg: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', $._expression),
    ),

    then_block: $ => seq(
      'then',
      repeat($._effect),
    ),

    // ── Effects ────────────────────────────────────────────────────────────

    _effect: $ => choice(
      $.assign_effect,
      $.compound_assign_effect,
      $.map_assign_effect,
      $.delete_effect,
      $.snoc_effect,
      $.emit_effect,
      $.return_effect,
      $.let_effect,
      $.if_effect,
      $.if_let_effect,
      $.match_effect,
      $.for_effect,
      $.sends_effect,
    ),

    assign_effect: $ => prec.right(seq(
      field('target', $._lvalue),
      ':=',
      field('value', $._expression),
    )),

    compound_assign_effect: $ => prec.right(seq(
      field('target', $._lvalue),
      field('op', choice('+=', '-=')),
      field('value', $._expression),
    )),

    map_assign_effect: $ => prec.right(seq(
      field('target', $.index_expr),
      ':=',
      field('value', $._expression),
    )),

    delete_effect: $ => seq(
      'delete',
      field('target', $._expression),
    ),

    snoc_effect: $ => prec.right(seq(
      field('target', $._lvalue),
      ':+',
      field('value', $._expression),
    )),

    emit_effect: $ => seq(
      'emit',
      field('event', $.identifier),
      '(',
      commaSep($.call_arg),
      ')',
    ),

    return_effect: $ => seq(
      'return',
      field('value', $._expression),
    ),

    let_effect: $ => prec.right(seq(
      'let',
      field('name', $.identifier),
      ':=',
      field('value', $._expression),
    )),

    if_effect: $ => seq(
      'if',
      field('guard', $._expression),
      'then',
      optional(field('then_label', $.branch_label)),
      repeat($._effect),
      optional($.else_clause),
    ),

    else_clause: $ => seq(
      'else',
      optional(field('else_label', $.branch_label)),
      repeat($._effect),
    ),

    if_let_effect: $ => prec.right(seq(
      'if', 'let', 'Some', '(', field('binding', $.identifier), ')',
      ':=', field('value', $._expression),
      'then',
      repeat($._effect),
      optional(seq('else', repeat($._effect))),
    )),

    match_effect: $ => seq(
      'match',
      field('scrutinee', $._expression),
      '{',
      sepBy1(';', $.match_arm),
      '}',
    ),

    for_effect: $ => prec.right(seq(
      'for',
      field('var', $.identifier),
      'in',
      field('iter', $._expression),
      'do',
      repeat1($._effect),
    )),

    sends_effect: $ => seq(
      'sends',
      field('msg', $.identifier),
      '(',
      commaSep($.call_arg),
      ')',
      optional(seq('to', field('dest', $._sends_dest))),
    ),

    _sends_dest: $ => $.indexed_path,

    indexed_path: $ => prec.right(seq(
      $.identifier,
      repeat(choice(
        seq('.', $.identifier),
        seq('[', choice($._expression, '*'), ']'),
      )),
    )),

    branch_label: $ => seq(
      '[',
      field('name', $.identifier),
      ']',
    ),

    _lvalue: $ => choice(
      $.identifier,
      $.field_access,
      $.index_expr,
    ),

    // ── Substrate block ────────────────────────────────────────────────────

    substrate_block: $ => seq(
      'substrate',
      field('name', $.identifier),
      optional(seq('realizes', field('surface', $.qualified_name))),
      '{',
      repeat($._substrate_item),
      '}',
    ),

    partial_substrate_block: $ => seq(
      'partial', 'substrate',
      field('name', $.identifier),
      optional(seq('realizes', field('surface', $.qualified_name))),
      optional(seq('owns', '{', commaSep1(field('field', $.identifier)), optional(','), '}')),
      '{',
      repeat($._substrate_item),
      '}',
    ),

    _substrate_item: $ => choice(
      $.doc_string,
      $.component_decl,
      $.replicate_decl,
      $.channel_decl,
      $.fairness_decl,
      $.auxiliary_block,
      $.authentication_block,
      $.maps_block,
      $.realizes_block,
      $.internal_block,
      $.acknowledged_block,
      $.epoch_decl,
    ),

    component_decl: $ => seq(
      'component',
      field('name', $.identifier),
      '{',
      repeat($._component_item),
      '}',
    ),

    replicate_decl: $ => seq(
      'replicate',
      field('name', $.identifier),
      '[',
      field('id', $.identifier),
      ':',
      field('id_type', $.type_expr),
      'in',
      field('id_set', $._expression),
      ']',
      '{',
      repeat($._component_item),
      '}',
    ),

    _component_item: $ => choice(
      $.doc_string,
      $.state_block,
      $.init_block,
      $.component_action_decl,
      $.receives_decl,
    ),

    component_action_decl: $ => seq(
      'action',
      field('name', $.identifier),
      $.param_list,
      optional(seq('->', field('return_type', $.type_expr))),
      optional($.when_clause),
      optional($.then_block),
    ),

    receives_decl: $ => seq(
      'receives',
      field('msg', $.identifier),
      '(',
      commaSep($.param),
      ')',
      'from',
      field('channel', $.identifier),
      optional($.when_clause),
      optional($.then_block),
    ),

    channel_decl: $ => seq(
      'channel',
      field('name', $.identifier),
      '{',
      'from',
      field('from', $._channel_endpoint),
      'to',
      field('to', $._channel_endpoint),
      '}',
    ),

    _channel_endpoint: $ => choice(
      $.identifier,
      $.channel_path,
    ),

    channel_path: $ => seq(
      $.identifier,
      repeat1(choice(
        seq('.', $.identifier),
        seq('[', choice($._expression, '*'), ']'),
      )),
    ),

    auxiliary_block: $ => seq(
      'auxiliary',
      '{',
      repeat(choice($.aux_decl, $.doc_string)),
      '}',
    ),

    aux_decl: $ => seq(
      field('kind', choice('history', 'prophecy')),
      field('name', $.identifier),
      ':',
      field('type', $.type_expr),
      ':=',
      field('init', choice($._expression, '*')),
      optional(field('cross_visible', 'cross_visible')),
      optional(seq('invariant', field('invariant', $._expression))),
    ),

    authentication_block: $ => seq(
      'authentication',
      '{',
      repeat($.auth_mapping),
      '}',
    ),

    auth_mapping: $ => seq(
      'surface_actor', 'of',
      field('action', $.qualified_name),
      '=',
      field('source', $._auth_rhs),
    ),

    _auth_rhs: $ => choice(
      'system',
      $.param_ref,
      $.indexed_path,
    ),

    param_ref: $ => seq('param', '.', field('name', $.identifier)),

    maps_block: $ => seq(
      'maps',
      '{',
      repeat($.maps_entry),
      '}',
    ),

    maps_entry: $ => seq(
      field('field', $.identifier),
      '=',
      field('value', $._expression),
    ),

    realizes_block: $ => seq(
      'realizes',
      '{',
      repeat($.realizes_entry),
      '}',
    ),

    realizes_entry: $ => choice(
      $.realizes_clause,
      $.for_some_realizes,
    ),

    for_some_realizes: $ => seq(
      'for', 'some',
      field('var', $.identifier),
      'in', field('set', $._expression),
      '.',
      $.realizes_clause,
    ),

    realizes_clause: $ => seq(
      'surface', '.',
      field('action', $.identifier),
      optional(seq('(', commaSep($._expression), ')')),
      repeat(field('selector', $.channel_selector)),
      'by',
      field('target', $._realization_target),
      optional($.when_clause),
    ),

    channel_selector: $ => seq(
      '[',
      choice(
        field('name', $.identifier),
        field('star', '*'),
      ),
      ']',
    ),

    _realization_target: $ => choice(
      'EXTERNAL',
      'stutter',
      $.realization_path,
    ),

    realization_path: $ => seq(
      $.identifier,
      repeat(choice(
        seq('.', $.identifier),
        seq('[', choice($._expression, '*'), ']'),
      )),
    ),

    internal_block: $ => seq(
      'internal',
      '{',
      repeat($.internal_entry),
      '}',
    ),

    internal_entry: $ => seq(
      $.identifier,
      repeat(choice(
        seq('.', choice($.identifier, '*')),
        seq('[', choice($._expression, '*'), ']'),
      )),
    ),

    epoch_decl: $ => seq(
      'epoch',
      field('name', $.identifier),
      '{',
      'advances_on',
      field('advances_on', commaSep1($._epoch_pattern)),
      'covers',
      field('covers', commaSep1($.identifier)),
      '}',
    ),

    _epoch_pattern: $ => $.indexed_path,

    acknowledged_block: $ => seq(
      'acknowledged',
      '{',
      repeat($.ack_entry),
      '}',
    ),

    ack_entry: $ => seq(
      field('kind', $.identifier),
      ':',
      choice(
        $.ack_list,
        $.ack_map,
      ),
    ),

    ack_list: $ => seq(
      '[',
      commaSep1($.ack_list_item),
      optional(','),
      ']',
    ),

    ack_list_item: $ => seq(
      $._ack_lhs,
      optional($.ack_because),
    ),

    ack_map: $ => seq(
      '{',
      commaSep1($.ack_map_item),
      optional(','),
      '}',
    ),

    ack_map_item: $ => seq(
      $._ack_lhs,
      choice(':', '->'),
      field('value', $._ack_rhs),
      optional($.ack_because),
    ),

    _ack_lhs: $ => prec.left(seq(
      $.identifier,
      repeat(choice(
        seq('.', $.identifier),
        seq('[', $._expression, ']'),
      )),
    )),

    _ack_rhs: $ => $._postfix_expr,

    ack_record: $ => seq(
      '{',
      commaSep1(seq(
        field('name', $.identifier),
        ':',
        field('value', choice($.identifier, $.qualified_name, $.string)),
      )),
      optional(','),
      '}',
    ),

    ack_because: $ => seq('because', ':', field('reason', $.string)),

    // ── Compose block ──────────────────────────────────────────────────────

    compose_block: $ => seq(
      'compose',
      field('name', $.identifier),
      '=',
      sepBy1('+', field('part', $.identifier)),
      '{',
      repeat($._compose_item),
      '}',
    ),

    _compose_item: $ => choice(
      $.doc_string,
      $.channel_decl,
      $.fairness_decl,
      $.realizes_block,
      $.acknowledged_block,
      $.epoch_decl,
    ),

    // ── Scenario ───────────────────────────────────────────────────────────

    scenario_decl: $ => seq(
      'scenario',
      field('title', $.string),
      repeat($._scenario_attribute),
      '{',
      repeat($._scenario_item),
      '}',
    ),

    _scenario_attribute: $ => choice(
      $.scenario_kind,
      $.scenario_tags,
      $.scenario_requires,
    ),

    scenario_kind: $ => seq('kind', ':', field('value', $.identifier)),

    scenario_tags: $ => seq(
      'tags', ':',
      '[',
      commaSep1(field('tag', $.identifier)),
      ']',
    ),

    scenario_requires: $ => seq(
      'requires_in', ':',
      '[',
      commaSep1(field('substrate', $.identifier)),
      optional(','),
      ']',
    ),

    _scenario_item: $ => choice(
      $.scenario_actors,
      $.scenario_given,
      $.scenario_when,
      $.scenario_then,
    ),

    scenario_actors: $ => seq(
      'actors',
      '{',
      commaSep1($.scenario_actor),
      optional(','),
      '}',
    ),

    scenario_actor: $ => seq(
      field('name', $.identifier),
      ':',
      field('type', $.identifier),
    ),

    scenario_given: $ => seq(
      'given',
      repeat1($._given_predicate),
    ),

    _given_predicate: $ => choice(
      seq($._expression, optional(';')),
    ),

    scenario_when: $ => seq(
      'when',
      choice(
        $.atomic_when,
        repeat1($.scenario_call),
      ),
    ),

    atomic_when: $ => seq(
      'atomic',
      '{',
      repeat1($.scenario_call),
      '}',
    ),

    scenario_call: $ => seq(
      field('actor', $.identifier),
      'does',
      field('action', $.identifier),
      '(',
      commaSep($._expression),
      ')',
    ),

    scenario_then: $ => seq(
      'then',
      repeat1($._then_predicate),
    ),

    _then_predicate: $ => choice(
      $.fails_with_clause,
      $.observed_clause,
      $.eventually_observed_clause,
      $.eventually_clause,
      seq($._expression, optional(';')),
    ),

    fails_with_clause: $ => seq(
      'fails', 'with',
      field('error', $.identifier),
    ),

    observed_clause: $ => seq(
      'observed',
      field('event', $.identifier),
      '(',
      commaSep($.call_arg),
      ')',
      optional(seq('by', field('actor', $._observed_actor))),
    ),

    eventually_observed_clause: $ => seq(
      'eventually',
      'observed',
      field('event', $.identifier),
      '(',
      commaSep($.call_arg),
      ')',
      optional(seq('by', field('actor', $._observed_actor))),
    ),

    eventually_clause: $ => seq(
      'eventually',
      $._expression,
    ),

    _observed_actor: $ => choice($.identifier, $.wildcard),

    // ── Attacker ───────────────────────────────────────────────────────────

    attacker_decl: $ => seq(
      'attacker',
      field('name', $.identifier),
      '{',
      repeat($._attacker_clause),
      '}',
    ),

    _attacker_clause: $ => choice(
      $.attacker_controls,
      $.attacker_initial,
      $.attacker_may,
      $.attacker_goal,
    ),

    attacker_controls: $ => seq(
      'controls',
      field('var', $.identifier),
      ':',
      field('type', $.identifier),
    ),

    attacker_initial: $ => seq(
      'initial',
      field('predicate', $._expression),
    ),

    attacker_may: $ => seq(
      'may',
      'any', 'action',
      'allowed', 'for',
      field('var', $.identifier),
    ),

    attacker_goal: $ => seq(
      'goal',
      'eventually',
      choice(
        seq('emits',
          field('event', $.identifier),
          '(',
          commaSep($.call_arg),
          ')',
          optional(seq('by', field('actor', $._observed_actor))),
        ),
        $._expression,
      ),
    ),

    // ── tla escape ─────────────────────────────────────────────────────────

    tla_block: $ => seq(
      'tla',
      '{',
      repeat(choice(/[^{}]/, seq('{', repeat(/[^{}]/), '}'))),
      '}',
    ),

    // ── Expressions ────────────────────────────────────────────────────────

    _expression: $ => choice(
      $.let_expr,
      $.if_expr,
      $.if_let_expr,
      $.match_expr,
      $._binary_expr,
    ),

    let_expr: $ => prec.right(PREC.let_expr, seq(
      'let',
      field('name', $.identifier),
      ':=',
      field('value', $._let_value),
      'in',
      field('body', $._expression),
    )),

    _let_value: $ => choice(
      $.if_expr,
      $.if_let_expr,
      $.match_expr,
      $._binary_no_in,
    ),

    _binary_no_in: $ => choice(
      $.binary_expr_no_in,
      $._unary_no_in,
    ),

    binary_expr_no_in: $ => choice(
      prec.left(PREC.or, seq($._binary_no_in, '||', $._binary_no_in)),
      prec.left(PREC.and, seq($._binary_no_in, '&&', $._binary_no_in)),
      prec.left(PREC.compare, seq($._binary_no_in, '==', $._binary_no_in)),
      prec.left(PREC.compare, seq($._binary_no_in, '!=', $._binary_no_in)),
      prec.left(PREC.compare, seq($._binary_no_in, '<', $._binary_no_in)),
      prec.left(PREC.compare, seq($._binary_no_in, '<=', $._binary_no_in)),
      prec.left(PREC.compare, seq($._binary_no_in, '>', $._binary_no_in)),
      prec.left(PREC.compare, seq($._binary_no_in, '>=', $._binary_no_in)),
      prec.left(PREC.compare, seq($._binary_no_in, 'is', $.identifier)),
      prec.left(PREC.compare, seq($._binary_no_in, 'subset', $._binary_no_in)),
      prec.left(PREC.set_op, seq($._binary_no_in, 'union', $._binary_no_in)),
      prec.left(PREC.set_op, seq($._binary_no_in, 'intersect', $._binary_no_in)),
      prec.left(PREC.set_op, seq($._binary_no_in, 'diff', $._binary_no_in)),
      prec.left(PREC.set_op, seq($._binary_no_in, 'cross', $._binary_no_in)),
      prec.left(PREC.add, seq($._binary_no_in, '+', $._binary_no_in)),
      prec.left(PREC.add, seq($._binary_no_in, '-', $._binary_no_in)),
      prec.left(PREC.mul, seq($._binary_no_in, '*', $._binary_no_in)),
      prec.left(PREC.mul, seq($._binary_no_in, '/', $._binary_no_in)),
      prec.right(PREC.compare, seq($._binary_no_in, '=>', $._binary_no_in)),
    ),

    _unary_no_in: $ => choice(
      $.not_expr_no_in,
      $.neg_expr_no_in,
      $.forall_expr,
      $.exists_expr,
      $.choose_expr_no_in,
      $.aggregate_expr,
      $._postfix_expr,
    ),

    not_expr_no_in: $ => prec.right(PREC.not, seq('not', $._unary_no_in)),
    neg_expr_no_in: $ => prec.right(PREC.unary, seq('-', $._unary_no_in)),

    choose_expr_no_in: $ => prec.right(PREC.quantifier, seq(
      'choose',
      field('var', $.identifier),
      choice(
        seq(':', field('type', $.type_expr)),
        seq('in', field('source', $._binary_expr)),
      ),
      '.',
      field('predicate', $._binary_no_in),
    )),

    if_expr: $ => prec.right(PREC.if_expr, seq(
      'if',
      field('cond', $._expression),
      'then',
      field('then', $._expression),
      'else',
      field('else', $._expression),
    )),

    if_let_expr: $ => prec.right(PREC.if_expr, seq(
      'if', 'let', 'Some', '(', field('binding', $.identifier), ')',
      ':=', field('value', $._expression),
      'then', field('then', $._expression),
      'else', field('else', $._expression),
    )),

    match_expr: $ => seq(
      'match',
      field('scrutinee', $._expression),
      '{',
      sepBy1(';', $.match_arm),
      '}',
    ),

    match_arm: $ => seq(
      field('pattern', $.match_pattern),
      '->',
      field('body', $._expression),
    ),

    match_pattern: $ => choice(
      seq('Some', '(', field('binding', $.identifier), ')'),
      'None',
      $.identifier,
      $.wildcard,
    ),

    forall_expr: $ => prec.right(PREC.quantifier, seq(
      'forall',
      commaSep1($.binder),
      '.',
      field('body', $._expression),
    )),

    exists_expr: $ => prec.right(PREC.quantifier, seq(
      'exists',
      commaSep1($.binder),
      '.',
      field('body', $._expression),
    )),

    binder: $ => choice(
      seq(
        field('name', choice($.identifier, $.tuple_pattern)),
        'in',
        field('source', $._binary_expr),
      ),
      seq(
        field('name', $.identifier),
        ':',
        field('type', $.type_expr),
      ),
    ),

    tuple_pattern: $ => prec(2, seq(
      '(',
      $.identifier,
      ',',
      commaSep1($.identifier),
      ')',
    )),

    choose_expr: $ => prec.right(PREC.quantifier, seq(
      'choose',
      field('var', $.identifier),
      choice(
        seq(':', field('type', $.type_expr)),
        seq('in', field('source', $._binary_expr)),
      ),
      '.',
      field('predicate', $._expression),
    )),

    aggregate_expr: $ => prec.right(PREC.quantifier, seq(
      'aggregate',
      field('body', $._postfix_expr),
      optional(seq('over', field('scope', $._binary_expr))),
      'using',
      field('aggregator', $._aggregator),
      optional(seq('else', field('else', $._binary_expr))),
    )),

    _aggregator: $ => choice(
      'exists', 'forall', 'sum', 'max', 'min',
      'union_set', 'union',
      $.identifier,
      $.concat_seq_aggregator,
    ),

    concat_seq_aggregator: $ => seq(
      'concat_seq',
      '(', 'order_by', field('order_by', $._expression), ')',
    ),

    // Binary expression chain: implicit precedence climbing.

    _binary_expr: $ => choice(
      $.binary_expr,
      $._unary_expr,
    ),

    binary_expr: $ => choice(
      prec.left(PREC.or, seq($._binary_expr, '||', $._binary_expr)),
      prec.left(PREC.and, seq($._binary_expr, '&&', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, '==', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, '!=', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, '<', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, '<=', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, '>', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, '>=', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, 'in', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, 'not', 'in', $._binary_expr)),
      prec.left(PREC.compare, seq($._binary_expr, 'is', $.identifier)),
      prec.left(PREC.compare, seq($._binary_expr, 'subset', $._binary_expr)),
      prec.left(PREC.set_op, seq($._binary_expr, 'union', $._binary_expr)),
      prec.left(PREC.set_op, seq($._binary_expr, 'intersect', $._binary_expr)),
      prec.left(PREC.set_op, seq($._binary_expr, 'diff', $._binary_expr)),
      prec.left(PREC.set_op, seq($._binary_expr, 'cross', $._binary_expr)),
      prec.left(PREC.add, seq($._binary_expr, '+', $._binary_expr)),
      prec.left(PREC.add, seq($._binary_expr, '-', $._binary_expr)),
      prec.left(PREC.mul, seq($._binary_expr, '*', $._binary_expr)),
      prec.left(PREC.mul, seq($._binary_expr, '/', $._binary_expr)),
      prec.right(PREC.compare, seq($._binary_expr, '=>', $._binary_expr)),
    ),

    _unary_expr: $ => choice(
      $.not_expr,
      $.neg_expr,
      $.forall_expr,
      $.exists_expr,
      $.choose_expr,
      $.aggregate_expr,
      $._postfix_expr,
    ),

    not_expr: $ => prec.right(PREC.not, seq('not', $._unary_expr)),
    neg_expr: $ => prec.right(PREC.unary, seq('-', $._unary_expr)),

    _postfix_expr: $ => choice(
      $.field_access,
      $.index_expr,
      $.call_expr,
      $._primary_expr,
    ),

    field_access: $ => prec.left(PREC.field, seq(
      field('object', $._postfix_expr),
      choice($._dot_id_token, $._dot_int_token),
    )),

    _dot_id_token: $ => alias(token(seq('.', /[a-zA-Z_][a-zA-Z_0-9]*/)), $.dotted_field),
    _dot_int_token: $ => alias(token(seq('.', /\d+/)), $.dotted_index),

    _tuple_index: $ => alias(token(/\d+/), $.tuple_index),

    index_expr: $ => prec.left(PREC.postfix, seq(
      field('object', $._postfix_expr),
      '[',
      field('index', $._expression),
      ']',
    )),

    call_expr: $ => prec(PREC.application, seq(
      field('callee', $._postfix_expr),
      $.call_args,
    )),

    _primary_expr: $ => choice(
      $.cardinality_expr,
      $.tuple_expr,
      $.set_expr,
      $.empty_brace_expr,
      $.record_expr,
      $.map_literal,
      $.comprehension,
      $.seq_literal,
      $.string,
      $.number,
      $.bool_lit,
      $.none_lit,
      $.some_call,
      $.wildcard,
      $.param_ref,
      $.identifier_expr,
      $.parenthesized,
    ),

    parenthesized: $ => seq('(', $._expression, ')'),

    cardinality_expr: $ => prec(PREC.cardinality, seq(
      '|', field('value', $._expression), '|',
    )),

    tuple_expr: $ => seq(
      '(',
      $._expression,
      ',',
      commaSep1($._expression),
      ')',
    ),

    set_expr: $ => seq(
      '{',
      commaSep1($._expression),
      optional(','),
      '}',
    ),

    empty_brace_expr: $ => seq('{', '}'),

    record_expr: $ => seq(
      '{',
      commaSep1($.record_field),
      optional(','),
      '}',
    ),

    record_field: $ => seq(
      field('name', choice($.identifier, $.string, $.number)),
      choice(':', '->'),
      field('value', $._expression),
    ),

    tuple_expr_key: $ => prec(2, seq(
      '(',
      $._expression,
      ',',
      commaSep1($._expression),
      ')',
    )),

    // Comprehensions:
    //   { x in s | P(x) }            -- filter
    //   { f(x) | x in s, P }         -- map / multi-binder
    //   { k -> f(k) | k in s }       -- map comprehension
    comprehension: $ => prec.dynamic(2, seq(
      '{',
      $._comp_head,
      '|',
      commaSep1($._comp_clause),
      '}',
    )),

    map_literal: $ => seq(
      '{',
      commaSep1($.map_literal_entry),
      optional(','),
      '}',
    ),

    map_literal_entry: $ => seq(
      field('key', $.tuple_expr),
      '->',
      field('value', $._expression),
    ),

    _comp_head: $ => choice(
      $.comp_map_arrow,
      $._expression,
    ),

    comp_map_arrow: $ => prec.right(3, seq(
      $._binary_expr,
      '->',
      $._expression,
    )),

    _comp_clause: $ => choice(
      $.comp_binder,
      $._expression,
    ),

    comp_binder: $ => prec(2, seq(
      field('name', choice($.identifier, $.tuple_pattern)),
      'in',
      field('source', $._binary_expr),
    )),

    seq_literal: $ => choice(
      seq('<', '>'),
      seq('<', commaSep1($._expression), '>'),
    ),

    bool_lit: $ => choice('true', 'false'),
    none_lit: $ => 'None',

    some_call: $ => prec(PREC.application + 1, seq(
      'Some',
      '(',
      field('value', $._expression),
      ')',
    )),

    wildcard: $ => '_',

    qualified_name: $ => prec.right(seq(
      $.identifier,
      repeat(seq('.', $.identifier)),
    )),

    qualified_expr: $ => prec(PREC.field, seq(
      $.identifier,
      repeat1(seq('.', $.identifier)),
    )),

    identifier_expr: $ => $.identifier,

    _call_target: $ => $.identifier,

    // ── Lexical primitives ─────────────────────────────────────────────────

    identifier: $ => /[a-zA-Z_][a-zA-Z_0-9]*/,
    number: $ => /\d+/,
  },
});
