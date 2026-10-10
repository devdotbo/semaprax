# Acyclic guard returns v1

Status: implementation authored; focused execution pending. The general
control-flow completion row and OPT #734 remain Partial.

A function body may end with `return value;`. It may also contain a returning
guard followed by the non-returning continuation:

```text
fn classify(value: i64) -> i64 {
    if value < 0 { return 1; }
    let doubled = value * 2;
    if doubled == 0 { return 2; }
    return doubled;
}
```

Each first guard arm must explicitly return on every path. Statements before
that arm's return are ordinary statements. Nested returning guards and
`else if` chains are admitted. An explicit final `else` must also explicitly
return on every path. A missing `else` moves the enclosing suffix into the
non-returning alternative exactly once. Ordinary non-returning statement
`if`s continue to use their existing grammar.

A terminal `match` may also use block arms that explicitly return on every
path. Mixed returning and ordinary arms are refused with `SPX-P210`; a suffix
after the terminal match is unreachable (`SPX-P209`). Match mode, scrutinee,
patterns, guards and exhaustiveness use the existing grammar and independent
source/HIR checks. Returning arms become ordinary value blocks, including
nested returning guards. This adds no general jump or loop-exit authority.

Canonical source intentionally projects this profile as existing value
control flow:

```text
fn classify(value: i64) -> i64 {
    if value < 0 { 1 } else {
        let doubled = value * 2;
        if doubled == 0 { 2 } else { doubled }
    }
}
```

The normalized AST is the authoritative meaning, including its lexical
scopes. The continuation is moved without cloning, and else-if alternatives
receive the same empty block wrapper as the existing value grammar. Parsing
canonical source therefore reproduces the same expression identities,
semantic graph, cleanup and loan plans, cache authority and target program.
Programs without an adjacent-operand `return` retain their original parser
route. A binding or call named `return` remains an ordinary identifier;
`return + 1`, `return - 1`, and `return(value)` keep their previous meaning.
To return a negative or grouped value without that ambiguity, use
`return { -1 };` or a named value.

This is syntactic normalization, with no new HIR exit authority, cleanup CFG
edge or target jump. Source verification, HIR validation and independent
cleanup/loan replay all see ordinary blocks and value `if`s. They continue to
check result types, consuming argument order, owned liveness, lazy effects,
sticky failure, provisional results and postconditions. A returned expression
must complete successfully before its enclosing function can publish a
result. Failure evaluating the expression or checking an `ensures` clause
keeps the ordinary selected-status path.

Code after an unconditional return is rejected with `SPX-P209`, not erased.
An explicit returning guard arm with a fallthrough value is `SPX-P210`.
Normalization checks the existing 128-level source nesting limit before
recursively materializing a continuation; final AST depth uses the same
independent source-depth gate and `SPX-P207`. No source, graph, cleanup or
materialization allowance is increased.

Returns from loop bodies, closure bodies, arbitrary nested value blocks or
initializers remain refused with the existing contextual `SPX-P106` hint.
`break`, `continue`, mixed fallthrough guard arms, labelled exits and general
control-flow proof schemas are not part of v1. This profile does not close
#734 or claim its owned loop-exit obligations.

`src/parser/return_flow.rs` owns normalization; its `match_tail` child reuses
the ordinary match-arm parser with an explicit returning-arm callback. The existing `language`
harness child `statement_if::guard_return` owns source/canonical/graph and
cleanup-plan equivalence, scalar and owned runtime paths, interpreter/native
and Core-Wasm behavior, terminal/nested guards, contextual refusal,
unreachable code and pre-materialization depth controls. The existing
`foreign_syntax_hints` module retains loop-return and ordinary-identifier
regressions. Run these owning selectors together after the implementation
batch; no benchmark improvement is claimed before a matched application run.
