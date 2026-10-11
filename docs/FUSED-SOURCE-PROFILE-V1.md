# Fused source profile v1

Status: authored source for OPT #768; executable qualification is pending. This
profile does not close the broader #747 ownership and backend obligations.

## Explicit source boundary

The versioned opt-in commands are:

```sh
semaprax run input.spx --profile fused-construction-v1 --managed-bytes 16777216 --json
semaprax run input.spx --profile fused-construction-v1 --managed-bytes 16777216 --native --json
semaprax build input.spx --profile fused-construction-v1 --managed-bytes 16777216 --target native --output fresh-program
semaprax build input.spx --profile fused-construction-v1 --managed-bytes 16777216 --target wasm --output fresh-package
```

The amounts above are explicit caller examples, not defaults or required-layout
auto-funding. `--managed-bytes` is mandatory and uses canonical positive decimal
syntax, from 1 through 1,073,741,824 inclusive. The upper bound is a new v1
administrative policy; it does not change an ordinary compiler or runtime cap.
Overflow, zero, leading zeros and unsupported option combinations are rejected
before source IO. The limit grants capacity, not an eager allocation of that
amount.

Only a source file with the exact independently selected closed construction
shape and one `main() -> i64` entry is eligible. The original unmodified
`tests/fixtures/record-traversal-maximum.spx` is the owning positive fixture.
The selected factory retains its existing distinction: a legacy construction
of at most 32 events follows ordinary admission; requesting this selected
profile for that shape is refused without ordinary fallback.
Names, paths, cached evidence and raw hashes grant no selection authority.
Project, test, check, graph, cache and ordinary run/build routes retain their
existing admission and diagnostics. Wrong profiles, arguments, exports, web
output and Project selection are refused. There is no fallback to ordinary
resolution after selected refusal.

The CLI reads the source once. The callback-free `semaprax::fused_source`
library facade parses once and holds its immutable AST while the private full
`ValidatedFusedSelection` is lent to the selected backend. Source SHA-256 is
computed inside the facade from those exact raw UTF-8 bytes. Public APIs accept
source text and finite options; they cannot accept HIR, Joined, Grant or a
replacement module. All receiving source/HIR, ownership, loan, cleanup and
status checks remain mandatory.

## Target accounts and fuel

| Target | Managed account | Exclusions |
| --- | --- | --- |
| Interpreter | Cumulative requested Rust layouts for runtime frame construction, execution and cleanup | Source parsing/compiler validation and fresh source/HIR joining, administrative account control, report serialization, allocator overhead and process memory |
| Native | Monotonic reservation of the complete closed requested heap footprint; structural allocations consume their held funding at original boundaries | Fixed caller/status/control stack, compiler, loader, libc/allocator overhead and output buffers |
| Wasm | Exact host adapter account contract supplied with the compiler-produced module and fixed host package | Account scope and exclusions must be declared by that package; no total JS/process-memory claim |

These amounts are not RSS or equivalent cross-target memory measurements.
Native categories, original effect boundaries, reentry, semantic status and
settlement are owned by [Native fused source account v1](NATIVE-FUSED-SOURCE-ACCOUNT-V1.md).
Wasm package qualification must establish its own precise funded categories,
module digest check, shared issuer and cleanup contract before support is
reported as implemented.

Interpreter execution retains the ordinary default 1,000,000 steps and accepts
`--max-steps` under the existing finite option policy. Native and Wasm do not
implement that interpreter fuel meter; native fuel options and build fuel
options are rejected rather than silently ignored. `--max-bytes` does not
replace the required managed account.

## Results and publication

Every selected invocation reports `semaprax.fused-source.v1`, profile, raw source
SHA-256, target, account limit, `managed_reserved` and outcome. A result appears
only for `returned`, after canonical publication and non-result cleanup. A
`language_failure` exposes the semantic status pair `{domain, code}`; interpreter
may additionally expose its full canonical `normalized_status`. Administrative
account, authority or runtime refusal has no invented language status or result.
Interpreter fuel/depth failures retain their own outcomes and cleanup evidence.
Cleanup cannot replace a selected status.

Native run emits a complete selected C unit before creating compiler scratch.
Native build publishes only into a fresh destination through the existing
identity-bound scratch/publication protocol. Wasm build publishes the module,
fixed host glue and compiler-derived manifest as one staged package; failed
selection or lowering must leave no publishable partial pair. The normal Wasm
execution entry accepts caller account limits, never substitute module or grant
bytes; the adapter verifies the expected module digest before instantiation.
Trusted first installation of the host namespace and cross-copy shared issuer
remain explicit assumptions, not properties of arbitrary supplied host code.

## Authored owning gates

The library owners are `fused_source::tests`: raw-source/default refusal and
original 4096 result; exact/one-short accounts, fuel and checked failure with no
poison publication; actual allocation observation through runtime frame Drop. The fixed test
observer has 16,384 tracking slots to retain the original 4,096 simultaneous
title allocations plus backing/frame/table allocations; tracking metadata is
excluded and exhaustion still invalidates the observation. This is test
instrumentation capacity, not a runtime/compiler quota.
`quickstart_v1::fused_source` owns real CLI source run/build and pre-IO denial
controls. Native receiver gates live in
`codegen::selected_fused_row::source_profile::tests`. Wasm and the consolidated
three-target receipt remain necessary qualification; authored gates are not
execution evidence. The broader hostile/profile/ownership matrix remains open
until all owning checks execute on the published source.
