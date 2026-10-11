# JSON codec suitability discovery v1

Status: source implementation; executable qualification pending. This additive
#773 surface does not close #722's archived-question replay and fresh-trial
measurement obligations.

`semaprax json-codec <project> --explain --type <stable-id>` is read-only. It
loads the ordinary authenticated Project, inspects its exact checked authored
type, and prints one compact JSON page. It accepts optional `--source <path>`,
`--target native64|wasm32`, `--max-string-bytes <1..64>`,
`--max-array-items <1..256>`, `--max-bytes <512..16384>`,
`--revision <project-revision>`, and `--offset <0..11>`. Options are unique and
unsigned values use canonical decimal spelling. Output defaults to 4096 UTF-8
bytes including the terminal LF. There is no output destination, plan,
publication, profile selection, candidate installation, provider or runtime
execution option.

An omitted source is discovered only by exact type identity among authored
manifest sources. Missing or ambiguous identities refuse; imported dependency
sources are not substitutes. An omitted target is inferred only from a singleton
manifest target matrix. Otherwise it stays null. Unknown targets refuse.
Manifest target permission is reported separately through the existing target
matrix predicate. It is not backend admission or a capability grant.

The library API is `project::explain_json_codec_suitability(&ProjectRevision,
&JsonCodecSuitabilityOptions) -> Result<String, Vec<Diagnostic>>`. Every call
rechecks the held revision. A supplied revision must equal the current Project
revision; every nonzero continuation offset requires that exact binding. This
prevents combining pages from changed source or manifests. The CLI additionally
uses the existing held-input recheck before returning output. The receipt binds
Project and workspace revisions, exact authored source path and SHA-256, selected
type identity, target and exact optional policy bounds. Each evaluated row
repeats its applicable bounds; omitted bounds remain null. No caller HIR or
report supplies authority.

Each page is an independent read-only query. Consumers combining pages must
compare the revisions, source path and digest, type, target and policy bounds
and require those fields to match. Changing a bound or target starts a different
query; `--revision` rejects source/manifest drift, not changed query parameters.

The `semaprax.json-codec-suitability.v1` envelope contains `profile_count: 12`,
`offset`, `coverage`, `next_offset`, and whole `profiles` rows. Order is the
compiler's existing codec inventory. `coverage` is `complete` only at its end;
otherwise `next_offset` identifies the next whole row. No matching or refused
profile is silently omitted. A byte budget unable to hold the binding and one
whole row refuses. Exact and one-byte-short budgets preserve this rule.

Each row uses the existing selector/bound descriptor and unchanged generator
predicate dispatch. Temporary internal fragments are inspected and discarded;
no replacement is returned or checked as a candidate. `shape_matches` is true
or false only after that predicate runs. Profiles needing omitted bounds instead
return null and complete `missing_policy` requirements. Predicates retain their
existing shape, original-permit, schema and generated-fragment limits. Refusals
carry their actual stable code and reason.

For a shape match, `required_modules` comes from its actual generated imports.
`imports_absent_from_current_entry_index` describes exactly which import IDs
are absent from the checked entry declaration index; it is discovery data, not
a complete dependency resolution or an import grant. Bundled/transitive modules
and aliases still undergo ordinary candidate resolution. Every row keeps
`candidate_admission` and `backend_admission` as `not_evaluated`; successful
shape rows name that remaining prerequisite. This version explains source-policy shape and manifest
target constraints; it does not claim which complete generated application will
pass any backend, grant a profile, or promise capacity, fuel or benchmark gains.

Existing derivation and codec-plan publication remain unchanged. Their full
candidate parse, canonical round-trip, dependency/signature/profile, ownership,
verification and target checks remain mandatory. Unsupported shapes and policies
retain their original refusals.

Owning checks live in `project::json_codec::suitability::tests` and
`cli::json_codec::suitability::tests`, with the existing codec derivation,
CLI/help and ordinary-source audit owners. They cover checked identity and source
receipts, actual missing-policy and import facts, candidate-refusal separation,
unknown/stale identities, exact/+1/one-short output and complete continuation,
strict inert CLI grammar, and native64/wasm32 manifest-policy distinction. No
executable or measured discovery improvement is claimed before qualification.
