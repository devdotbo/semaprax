# Selective owned npm lexical semantic recipe v1

Status: authored bounded contract; executable qualification remains separately gated.
Audience: coding agents, compiler contributors and reviewers.

The pathless owned-data npm construction used by the AgentLifecycle Core Wasm
executor can select `semaprax.owned-data-semantic-recipe.lexical.v1` for linked
functions containing explicit lexical control. This is a source replay profile,
not a runtime import, capability, cleanup grant or new public result ABI.
No npm process runs during this preparation.

Selection is the exact leading source comment
`// semaprax.owned-data-semantic-recipe.lexical.v1`. Ordinary linked functions
retain their historical recipe-v8 bytes and validation path. Frozen v8 rendering
and replay still refuse lexical expressions and lexical loops. A marker on an
ordinary recipe is refused; absence of the marker cannot admit lexical control.

The selected profile independently validates the complete linked HIR before
rendering. Replay parses and resolves ordinary source, independently validates
its HIR and cleanup certificates, and requires canonical re-rendering, exact
persistent declaration/function inventory and entry identity. Existing package
replay then re-derives its public descriptor and every generated artifact,
including exact Wasm bytes. The marker alone never authenticates an exit target,
callee, loan, latch or cleanup domain. Carrier digests bind the complete marked
recipe bytes through the existing envelope; no frozen carrier field is omitted.

The initial profile retains ordinary condition-latched `while`, explicit
`break`, `continue`, `return <value>;`, and independently admitted unread scalar
statement-if guards. Rendering a guard preserves statement position, rather
than introducing a forbidden initializer crossing. Typed generated source-for
latches and statement-match guards remain refused in this profile. Existing
recipe limits, function-value/import/effect/contract refusals and ordinary type
and public descriptor restrictions remain unchanged. No ownership or failure
rule is widened. String allocation failures retain their existing contract.

Owning authored regressions are
`project::npm::semantic_recipe_lexical_v1::tests::` (two bodies). They cover
canonical guard/exit replay, independent Wasm equality, unchanged ordinary
recipe bytes, frozen-v8 refusal and a forged target. The existing real
AgentLifecycle lexical fuel/cleanup/reentry test owns the executor integration.
These definitions do not constitute executed qualification evidence.

See [structured exits](LEXICAL-STRUCTURED-EXITS-V1.md) and
[owned npm invocation](OWNED-NPM-INVOCATION-V1.md) for control and invocation
settlement contracts.
