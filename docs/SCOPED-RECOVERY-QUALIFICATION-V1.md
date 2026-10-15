# Scoped recovery qualification v1

Status: source-only; current-head qualification pending.

Audience: compiler contributors.

This private note records the receiver and cold-owner adaptations made after two
qualification selectors failed against the frozen source. It changes no runtime
prelude declaration, selection, schema or byte, and makes no runtime fit,
savings, token or support claim. No Cargo, build or test command was run while
authoring it.

## Intentional wrong-field delta

`SCOPED-DIAGNOSTIC-RECOVERY-V1` intentionally suppresses a repeated `SPX-T202`
for a read of a pattern binding whose field was refused. For the corpus fixture
`match (Option<i64>::Some { value: 1 }) { Option::Some { wrong: n } => n, ... }`
inside `while false`, the unchanged recursive oracle and the saved legacy
iterative receiver report three diagnostics (`SPX-M104` unknown field,
`SPX-M104` missing payload field, `SPX-T202` read of `n`); the current receiver
reports the first two. Production recovery is unchanged.

The original corpus fixture literals, recursive oracle and `recovery_legacy`
receiving bodies remain unchanged. The receiving helpers `compare_scalar_body` in
`iterative_verifier_tests` and `constructor_tests::corpus` become thin wrappers
over `compare_scalar_receivers(source, intentional)`. Only an exact full-byte
match of the pinned fixture selects the intentional branch, which delegates to
`recovery_tests::intentional_wrong_field`. That module asserts the full six-field
old and current diagnostics (code, severity, message, path, span, help) and
their order for the recursive, legacy and current receivers and for the public
current and old declaration passes. Every other corpus case keeps strict
old/current equality, and the value and scope-state checks still run on the
pinned fixture.

The module also carries bounded semantic controls in ordinary, loop and
traversal contexts: unknown and duplicate field names, duplicate binding names,
valid reads, a wrong case, independent unbound identifiers, guards, sibling arms,
refused lets and closures. Changed cases independently name full six-field
Diagnostic objects at exact authored read sites and preserve every other
diagnostic in order; no receiver is normalized. Unchanged cases require full
equality.

## Cold process-owner acquisition

`prelude::declarations()` is a process-lived `OnceLock<Vec<TypeDeclaration>>`
initialized lazily by the first verifier or type table. A cold acquisition
inside the measured invocation retains that process owner beyond invocation
Drop. The actual failure receipt does not distinguish invalid observation from
live backing, so attribution remains source-derived and pending qualification.
The measured controls now hold only that immutable table before measuring. They
never warm the verifier, type table or full receiver, and the allocation
observer, capacities and Drop assertions are unchanged; failure messages now
print the metrics.

Cold acquisition is a separate scope. It is process-lifetime state shared by the
test process, so its control accepts either a warm or a cold outcome and checks
the retained owner explicitly. The same owning construction is measured
independently through Drop with allocations equal to frees and zero live bytes.
The prelude vector initializer is factored into a private owning factory used
by the `OnceLock`, and a test-only copy of the original initializer body is the
reference it must match.

## Fixture controls

Nine recovery selectors failed at runtime: eight contain invalid source fixtures,
and one observes a process-lived prelude owner inside an invocation Drop scope.
Production recovery is unchanged.

- Missing final arm commas: the original bodies stay literal in
  `recovery_tests/fixture_controls.rs` and are asserted as parser `SPX-P106`
  refusals with all six fields, the site derived from the literal's closing brace.
  Corrected trailing-comma bodies keep full receiver, declaration-pass,
  canonical and graph parity.
- Local `result`: the original keeps full old and current sequences including
  `SPX-S109` and `SPX-T201`. The `outcome` counterpart keeps the fixed-result
  reset with its own full sequences.
- Non-`i64` or parameterized `main`: originals keep exact `SPX-T104` (and
  `SPX-O002` for `own string`) objects with spans from the parsed declaration or
  literal site. Counterparts use a named `probe` helper, a valid `main`, and
  `owner: string`. Existing direct-body receivers and `state` checks are unchanged.
- Boolean `match`: the original missing catch-all retains the complete authored
  `SPX-T257` object and exact old/current equality under Refutable Match v1.
  Admitted Boolean and integer counterparts retain canonical and graph parity.
- Direct return: the original missing-semicolon source retains its complete
  authored `SPX-P106` object under Acyclic Guard Returns v1. The admitted helper
  preserves full old type/ownership symptoms and the exact current refusal view
  without fabricating ownership for a mismatched actual carrier.
- Allocation: measured controls hold only the immutable prelude table and
  measure the full invocation through Drop, asserting alloc == freed, live 0,
  not invalid, and a stable held pointer and length. Failure messages print the
  metrics. No attribution claim is made.

All of this is authored only; no Cargo, build or test command was run.

## Pending selectors

- `--lib source_verify::iterative_verifier_tests`
- `--lib source_verify::constructor_tests`
- `--lib source_verify::recovery_tests`
- `--lib prelude::tests`
- `--test module_size`
- `--test source_locked_contracts`
