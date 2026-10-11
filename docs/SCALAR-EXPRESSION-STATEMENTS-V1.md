# Scalar Expression Statements v1

Status: authored implementation; focused qualification pending.
Audience: SEMAPRAX authors and compiler contributors.

An intermediate `expr;` discards exactly one existing Copy scalar: `i64`,
`i32`, `u8`, `usize`, `char`, `f32`, `f64`, or `bool`. There must be a following
block item. A block still requires its final value; `expr; }` retains `SPX-P106`.
Owning values, aggregates, functions and borrowed views require an explicit
binding (`SPX-T315`). Operand typing and its diagnostics run first.

The parser normalizes the statement to an immutable unannotated ordinary let.
Its fresh `_if<n>` name skips every identifier in the whole file, including
later text and nested scopes, using the statement-if name issuer. `_` remains
an ordinary binding: repeated `let _` retains `SPX-T209`.

`LetSyntax::ExpressionStatement` appends AST cache tag 3; existing tags retain
exact bytes. The marker grants no HIR, cleanup, loan, capability, lexical-exit
or graph authority. Both source verifiers independently check its shape,
actual scalar type and Value mode. HIR resolves the ordinary let and all
backends execute its ordinary left-to-right operand, staged calls and cleanup.
Lazy boolean operands stay lazy. No owning-result discard is introduced.

Canonical formatting preserves `expr;` only for structurally consistent
markers whose binding has no readers or competing declarations in the enclosing
body. One bounded borrowed-name census is shared by nested writers; forged
readers, duplicates, mutability or annotations preserve the full let spelling.
Name scratch is reserved before growth; global-budget refusal remains sticky.
The hidden structural AST helper is presentation support, never typed authority.

A generic discarded initializer cannot contain a lexical exit (`SPX-T312`).
The existing separately authenticated StatementIf marker keeps its statement
exit privilege. See [lexical exits](LEXICAL-STRUCTURED-EXITS-V1.md),
[statement if](STATEMENT-IF-CANONICAL-V1.md), and
[language ergonomics](LANGUAGE-ERGONOMICS-V1.md).

Owning gates: `language::statement_if::scalar_expression::` (six authored
bodies), the existing statement-if/foreign-syntax controls, and AST carrier
cache round-trip. These cover all eight scalars, hygiene, canonical/graph
replay, forged markers, owning/borrowed refusals, prior diagnostics, consumed
String arguments and lazy checked failure on ordinary backend routes.
Qualification and application/cost evidence remain separate obligations.
