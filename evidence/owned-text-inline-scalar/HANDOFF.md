# Whole owned-text scalar receiver and selection fixture

Base: `31dd9324`. Changes only owned_text_record/paid.rs and its existing tests.
The saved text and collection sources, all archived-body hashes, original Drop
observers, strict `old.allocated > new.allocated`, physical-paid inequalities,
limits, and old/current classifier parity remain intact. Existing builder and HIR
source inventories already include both files; no new Rust descendant is added.

Actual frozen fde failures: owning-selected-90.log selected unknown `String` in
the selection fixture, and owning-selected-91.log failed the full saved/current
scalar allocation inequality. These failures are not claimed qualified here.

The exact uppercase `String` source survives as ORIGINAL_SELECTION_SOURCE with a
new exact T001 refusal and complete checked-source/resolve diagnostic-vector
comparison. The existing source-hash/program-selection owner uses an explicitly
checked lowercase `string` counterpart, retaining the original declaration IDs,
field, program shape and full old/current program-selection assertion.

The saved scalar scan acquires a one-element Cow Vec for its initial root. The
paid scan previously held the root inline but acquired at least four Cow slots
for the one scalar child. Reducing first capacity to one would merely equalize
backing, rather than satisfy the strict original removal assertion. The current
scan now reuses its already-live inline Cow slot for the last scheduled child.
When another child arrives, the earlier inline child spills to the paid Vec,
reserved before acquisition. Substitution still runs in original authored order;
children still execute last-authored first; existing pending siblings remain below
the current node's children; the complete structural visited tree is unchanged.
Every actual Vec acquisition and visited node remains prepaid, and zero-byte
sticky ledger authentication remains at entry and all spill reservations.

The new full-saved receiver control spans 0/1/2/17 scalar fields, actual Drop,
strict allocation reduction, paid physical backing, exact/one-short/sticky checks,
and existing complete parity. For a singleton it asserts the exact removed
request equals size_of::<Cow<ResolvedType>>(), with no guessed platform byte size.
No diagnostic allocation exclusion, offset normalization, credit, cap increase,
or prewarming is used. This is a source-derived backing improvement pending
combined qualification; no total-app fit/performance claim is made.

Source-only checks: rustfmt and git diff --check. No Cargo/build/test/full gate.
Exact pending owning selectors and immutable saved-source file hashes are listed
in pending-selectors.json. The separate executable_profiles uppercase fixture is
owned by Sol and is not changed here.
