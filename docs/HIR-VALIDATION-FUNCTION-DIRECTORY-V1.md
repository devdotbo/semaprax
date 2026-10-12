# HIR validation function directory v1

This private storage change gives no new source, HIR, cache, selected-profile or
backend authority. Qualification is pending.

The complete HIR inspection caller and independent cleanup execution caller
prepare one directory borrowing the exact current `ResolvedProgram`. Its owned
Vec contains one original ordinal and function reference per ordinary function;
function instances retain their existing separate validation. The same held
caller now also retains a sorted Vec of borrowed module-permit strings, charged
once as permit_count * sizeof(&str) before construction. Duplicate permit values
are retained in this directory; membership alone matches the old set and never
replaces independent source/declaration checks. Closure validators reuse the
same permit metadata but still perform current function effect/body checks.
 The caller
reserves the exact requested entry backing on the existing active builder
before allocation, with no new ledger, allowance or refund. With no active
builder, the same private constructor retains the existing unobserved route.
Allocator failure and capacity mismatch poison the same active owner. Actual
excess capacity is debited after the allocator reports it, before rejecting the
mismatch; no later zero-unit or fitting request restores admission. Tests inject
these refusal continuations after actual directory acquisition, rather than
claiming the platform allocator was forced to return excess capacity.
Initial storage refusal is inline. The enclosing builder's first requested-byte
receipt remains authoritative; the compatibility Diagnostic has SPX-H006 and an
empty message rather than constructing unfunded text.

Allocation-free sorting orders only lookup metadata by ID and original ordinal.
The program and canonical plans are unchanged. The pure validator receives no
reservation callback or physical capability. It runs the original source-order
function checks, including reserved operation identity, first duplicate and
exact declaration kind/name agreement. An earlier declaration mismatch still
precedes a later duplicate. Every lookup checks the held program and actual
function pointer; equal text or a same-cardinality cloned program cannot lend
membership. Closed directory fields have no caller constructor or mutation API.

Closure validation reuses that directory, but still reconstructs and validates
its current closure function and all its ordinary signatures, bodies and
contracts. The old owned map constructor is retained only for independent
unit-reference validation. This storage change does not cache a HIR verdict.

The two lib owners are
`hir::validation::function_lookup::tests::borrowed_lookup_matches_owned_order_duplicates_and_exact_program_members`
and
`hir::validation::function_lookup::tests::caller_function_directory_has_literal_exact_short_sticky_payment_and_full_drop`.
The first compares saved source-order lookup/diagnostic behavior and complete
old/current core verdicts. The second observes actual directory acquisition,
lookup and Drop together: one allocation of `function_count * (sizeof(usize) +
sizeof(function-reference))`, exact/one-short/zero and same-ledger refusal retry.
Program, fixed ledger context and observer metadata are outside that scope.
The old owned-map allocation observation is separate. These are authored source
controls, not executed results.

This funds and removes only the function lookup's old ID/tree allocations.
Canonical loan reconstruction, origin/endpoint directories, validation scopes,
branch clones, type/permit/effect/call collections, other replay children and
owned semantic diagnostics still need their actual payment joins. No whole
validator funding, application fit, timing gain or default cap change is claimed.
