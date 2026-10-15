# Private source admission checkpoint — not integration ready

Basis a2311ae7. Existing verifier frames/scopes supply byte-only ancestry and
outer-entry typed view lookups; no new verifier/frame field or owned snapshot.
Non-byte/mixed/computed/mutable/unavailable/inside-body sources retain their
old early rejection. Closure boundaries clear admission ancestry. Source freeze
acquire/release remains unchanged and each nested loop has a distinct old ID.

Complete original loop-admission implementation is archived and independently
compiled in before_nested, with only exact method-name/visibility adaptation.
Three source-only controls are authored: positives/canonical round-trip, complete
old diagnostics on closed routes, unchanged nonnested outcomes/test-scope unwind.
No Cargo/test execution.

This checkpoint is INCOMPLETE: HIR still has the original nested-receiver
refusal; contextual census, receiver consumers, source profile spec/help,
physical/backend/cache controls and full joined source custody remain pending.
Do not integrate or claim #780 implemented from this checkpoint.
