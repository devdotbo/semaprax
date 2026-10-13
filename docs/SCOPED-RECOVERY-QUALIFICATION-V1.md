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

## Pending selectors

- `--lib source_verify::iterative_verifier_tests`
- `--lib source_verify::constructor_tests`
- `--lib source_verify::recovery_tests`
- `--lib prelude::tests`
- `--test module_size`
- `--test source_locked_contracts`
