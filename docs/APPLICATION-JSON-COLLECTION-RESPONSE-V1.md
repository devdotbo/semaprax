# Bounded application collection response source v1

Status: authored bounded contract; executable qualification remains separately gated.
Audience: coding agents, compiler contributors and reviewers.

This additive **encoder-only source selector** is
`bounded-collection-response.v1`, selected with a canonical decimal
`--max-string-bytes` from 1 through 64. The API profile is
`JsonCodecProfile::CollectionResponse { max_string_bytes }`. Source and runtime
qualification are pending the owning executable gate. The selector does not
grant v30 command admission, a public nominal ABI, or authority to install HIR.
It is a finite #724/#727 tranche, not a general recursive JSON codec.

The selected authenticated source must contain an explicitly identified,
monomorphic record with exactly two fields in either declaration order: one
direct `Vec<Row>` and one flat scalar `Metrics` record. Both child records must
be declared in that same source. Row has one or two `string` fields and zero
through six `i64`, `u8`, `usize` or `bool` fields, with at most eight fields
total. Metrics has one through eight of those scalar fields. Record and field
identities are explicit, and no type parameters or invariants are admitted.
Existing source/name/schema/generated-byte limits remain unchanged. Field
names, type names and identities are arbitrary admitted source names; a
benchmark-specific name or generated origin grants no exemption.

For `Report { items: Vec<Item>, metrics: Metrics }`, generated APIs are:

```text
json_Report_collection_response_encoded_len(value: borrow Report) -> usize
json_Report_collection_response_encode(value: borrow Report,
                                       output_limit: usize)
    -> ReportJsonCollectionResponseEncode
```

Their identities are `<report-id>.json.collection-response.encoded-len` and
`<report-id>.json.collection-response.encode`. The ordinary affine outcome,
identified by `<report-id>.json.collection-response.encode-result`, is
`Encoded { text: string }` or `Refused { required: usize }`.

The encoder emits compact JSON in each record's **declaration order**. Vector
order is preserved, without sorting, repairing, deduplicating or reinterpreting
application data. Business sorting can use a different ordinary record and map
named fields to the response record before encoding. Integers retain their
complete exact ranges; booleans emit lowercase literals. Strings are validated
as UTF-8, including NUL and every Unicode scalar, then quote/backslash and
control bytes use deterministic JSON escapes. Other Unicode scalars remain
literal UTF-8. The bound is independently applied to every stored Row String's
UTF-8 byte length. Every String field is checked in declaration order before
output allocation. Empty/repeated Strings and an empty vector are valid; domain
uniqueness and application numeric ranges remain the caller's responsibility.

Preflight checks the actual vector length (at most 256), stored String validity
and byte bound, and exact encoded UTF-8 byte length. Actual output over 131072
bytes, invalid Strings or excessive cardinality yield `usize::MAX`; encoding
returns `Refused { required: usize::MAX }`. A valid one-short caller output limit
returns the exact required byte count before response String construction.
The count/string maxima are not a guarantee that every possible combination of
long field names fits the output cap: such a value is refused explicitly.
No physical, fuel, borrowed-root or ownership bound is increased.

The helper borrows the actual Report owner for the complete call. Its ordinary
projected vector reads independently authenticate the full field path. Both
preflight and rendering call indexed Row helpers over `borrow Vec<Row>` and a
Copy index; `vec_field<Row>(values, at, "field")` reads scalars or nonescaping Str
views from the retained vector generation. Exact field identities and full
parent paths remain ordinary compiler and loan-replay obligations. Neither pass
clones the Report, Vec or stored Row. Direct `borrow Row` length helpers remain available; rendering helpers now
add the ordinary owned Bytes output and scalar position; the root encoder selects the indexed
helpers with deterministic `.object-len-at` and `.object-render-at` identities.
Metrics is Copy and passed by value.

Preflight creates no owned Row or String result. Rendering reserves one exact-sized Bytes payload and fills it after the
complete preflight and caller limit check, then performs final checked UTF-8
String materialization. The old source made two Row deep copies per encoded element: at
256 rows that is 512 Row clones, or up to 1,024 String leaf clones for two-String
rows. These are source-derived avoided operations, not measured allocation,
latency, token or cost savings. The scoped read contract adds no authority or
profile widening. Explicit caller `vec_clone_at` remains an allocating owning
operation, with its original failure and cleanup semantics.

Normalized allocation failures retain ordinary argument staging, group commit,
sticky failure selection and partial-owner cleanup; they are not converted into
a JSON refusal or published partial output. Native String allocation is fatal
outside that status channel; this selector does not establish recovery or
zero-leak settlement at those fatal sites. The owning
regression keeps an explicit post-preflight Row clone and its injected status15
failure, while a separate positive witness forbids deep String clones during
repeated preflight and successful encoding of two-String rows.

Derivation and replay rebuild the caller's unchanged Project from its exact
authenticated revision, selected stable type identity, policy and bound.
Canonical source round-trips and ordinary type/HIR/ownership/profile/backend
checks remain mandatory before exclusive CLI output publication. Missing child
types, stale output, a changed bound or an unsupported schema refuse as SPX-J180.
Generated source carries no filesystem/process/network authority and has no
special helper-origin rule. Nested collection runtime support belongs to the
independent ordinary compiler carrier contract; this selector cannot broaden
frozen v29/v30 command profiles.

Owning source gates are `project::json_codec::collection_response::tests::` (4) and
the Project harness's
`standard_library::application_json::collection_response::` (7). They cover exact
source/policy replay, wrong schema refusal, declaration/wire order, Unicode/NUL,
all scalar boundaries, exact/one-short output, full cardinality, malformed
values, allocation-free Row reads and strict explicit-clone cleanup, with the same admitted application
source on interpreter, native C11 O0/O2 and strict Core Wasm. No executable
acceptance, tokenizer saving or causal cost result is claimed before those
gates and a fresh source/binary-bound application qualification.

## Exact bounded writer composition (OPT-737)

Response rendering uses ordinary source helpers that thread `own Bytes` and a
scalar position. The original manifest, dependency inventory and selected
profile remain authoritative; derivation does not inject `std.io` or
`std.format`. Those packages' Writer representation is not assumed to compose
with every response profile. The generated helpers use the existing checked
byte-write operations and the separately checked bounded allocation operation.
They receive no authority from their generated names or identities.

The full existing preflight runs before reserving the buffer. Invalid schema
values and exact-size-minus-one caller limits return the original `Refused`
outcome without reservation or publication. Every helper additionally checks its
own exact length and remaining capacity before its first write. Keys are fixed
Copy byte arrays in short scopes, without temporary owned Strings. Numeric digits
are written directly, including the minimum signed value without negating it;
booleans and JSON punctuation retain their exact spelling. Valid UTF-8 bytes are
copied directly, with the existing short escapes and lowercase `\u00xx` controls.
Field declaration order, array order, bounds, ordinary fuel and cleanup authority
remain unchanged. Helpers never grow, replace, or clone the payload buffer.

`string_from_utf8(bytes_as_slice(finished))` performs the final checked copy into
an owned String. This is one buffer reservation plus one String materialization,
not a zero-copy or single-allocation result. Buffer zero-fill, per-byte stores,
final payload copy and allocator/header work are distinct costs. Current native
`string_concat` already uses one allocation; OPT-737 targets repeated prefix
copying and temporary materialization, not a historical double-buffer claim.

Ordinary normalized bounds/write failure still follows canonical
sticky-status cleanup. Fatal native String allocation remains outside that
recoverable channel. Preflight does not make native String OOM recoverable or
prove zero-leak settlement there; that OPT-724 obligation remains open.

The new owning definitions retain the original independent wire oracles and
same-source interpreter/native O0/O2/Core-Wasm requirements. A separate native
quote-phase definition observes exact buffer reservation, final String allocation
and explicit memcpy traffic for empty, 16/64-byte ASCII, Unicode, controls and
quote/backslash inputs in one context. It is not a full application, full-row
physical measurement, accepted fresh trial, billed cost, or an executed gain.
Executable qualification and matched application trials remain pending.

An opt-in physical-work definition lives in the existing native harness at
`string_settlement::bulk_utf8::collection_writer::generated_collection_response_exact_physical_work`.
It derives and verifies the actual selected codec, independently validates its
linked HIR and repeats deterministic C emission. Native O0/O2 run the same
emitted code and immutable byte oracle. Rows 0, 1, 8 and 256 cover empty,
intermediate and actual maximum cardinality; all controls, NUL, quotes,
backslashes and multi-byte Unicode occur in each nonempty row. Inputs are built
before observation and remain alive across preflight, one-short refusal and
exact encoding. No runtime allocation/fuel limit is reset.

The probe observes exact Bytes reservation, ordinary String physical allocation,
explicit `memcpy` traffic and the final buffer-to-String payload copy separately.
Other carrier traffic is retained in total explicit-copy counts. Scalar byte
stores are not labeled as measured copies; their count remains null. A second
unmeasured authentic render checks retained input semantics, then canonical
cleanup must balance the complete physical owner inventory. The external
receipt retains source/test-executable hashes, generated source, independent
oracles, compiler/run logs and actual metrics. This is an authored measurement
route, not executed evidence or an old/current application comparison.

Use a fresh external `SEMAPRAX_COLLECTION_WRITER_OUTPUT` directory and explicitly
select this ignored owning test when the coordinated measurement phase is
permitted. The ordinary fatal String OOM path prevents interpreting this probe
as a returned-allocation fault matrix; production recovery obligations remain
open under OPT-724. Application/cost qualification for OPT-737 remains separate.
