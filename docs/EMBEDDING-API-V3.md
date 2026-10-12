# Embedding API v3

Status: source-authored Rust facade migration; executable qualification pending.
The completion matrix owns broader product status.

Version 3.0.0 retains the operations, explicit capabilities, outcome selection,
limits, cancellation, panic normalization and authority boundaries specified in
[Embedding API v2](EMBEDDING-API-V2.md). That document remains the historical
major-2 contract. This document owns the following public Rust type change.

## Diagnostic message ownership

`Diagnostic.message` changes from `String` to `Cow<'static, str>`. This is a
Rust source incompatibility: struct literals must wrap dynamic strings with
`Cow::Owned`, consumers requiring a `String` use `into_owned()`, and text reads
use `as_ref()`. Mutating text uses `to_mut()` and may allocate for borrowed text.
The public `error`, `warning` and `io` constructors still accept `Into<String>`;
nonstatic caller slices remain accepted, and a moved String retains its exact
pointer and capacity. Their messages remain owned. The private static compiler
constructor accepts only static text and stores a borrowed literal.

`Diagnostic::message_owned_capacity()` reports the actual owned String capacity,
or zero for a borrowed static message. It does not replace capacity by length
or price diagnostic vectors, path/help storage, clones of owned text or output.
Fixed generic-value, ordinary scope and owned iterator-frontier refusal messages
use the static constructor, preserving their full text and first-error order.
Their zero-entry error creation, clone and Drop require no message allocation.
Dynamic failures, rendered outputs and report-vector funding remain separate
obligations; this migration does not complete OPT781 or OPT795.

## Compatibility policy and encoding

`EMBEDDING_API_VERSION` is `3.0.0`. The existing exact-major policy accepts 3
and refuses 2, 1 and 0; major 2 is retained as a historical matching/refusal
control. Hosts must negotiate 3 before invoking this Rust facade. Negotiated
facade descriptors that encode this constant consequently change from 2 to 3.

Diagnostic JSON fields, schema versions, escaping, Display bytes and error
selection remain unchanged. Report/wire schema versions independent of the
Rust facade version are retained. This does not claim every descriptor byte
is unchanged or establish a stable C ABI. Future breaking changes to existing
Rust outcome types still require a facade major bump under the v2 policy.

## Source-authored owners

The complete prior diagnostic tests and saved ordinary/generic receiver oracles
remain. New owners in `diagnostic::refusal_tests` cover zero-entry message
creation/clone/Drop, dynamic nonstatic and prepaid-owned pointer/capacity, and
full JSON/Display/Debug parity for all location shapes. Generic and scope
refusal modules cover arithmetic, exact, one-short and sticky full receiving
lifetimes; the iterator frontier owner covers its actual entries through Drop.
No execution or whole-application funding advantage is claimed for this tranche.

## Additive affine source-preflight failures

The source-authored public `WorkspaceReportFailure` and
`semantic_workspace::reporting` API are additive to facade major 3 and specified
by [Workspace report owner v1](WORKSPACE-REPORT-OWNER-V1.md). They preserve an
initial refusal or existing paid selected-replay owner without implicitly
materializing a Vec. Existing outcome types, Vec APIs, negotiated version,
wire schemas and encoded bytes remain unchanged. Source preflight has no
filesystem or publication authority; remaining filesystem/directory/output
funding is explicit and qualification is pending.
