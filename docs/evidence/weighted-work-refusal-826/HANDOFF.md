# OPT826 detached work refusal control correction

Status: source-authored correction; no compiler or tests executed.
Audience: compiler contributors and reviewers.

Actual current350 selector328 failed the old assertion that a detached
`ReplayBudget::new()` must reject a zero-unit charge after work exhaustion.
`ReplayBudget::charge` delegates exhaustion to the complete current
`budget_refusal`. For `LegacyUnobserved`, that function returns the original
owned Diagnostic before changing `refusal_spent`, `remaining`, or the skeleton
counter. Both the saved complete canonical receiver and the shared receiver
therefore retain the same raw zero-charge result. This compatibility counter is
not an affine selected/ActiveBuilder/Oracle ledger. Those existing funded
owners retain their actual sticky refusal controls and implementation.

The corrected owning selector keeps the full old/current first Diagnostic,
work use, exact/one-short/zero boundaries, and compares the complete subsequent
zero-charge result and unchanged counter/refusal state. It also binds the
actual ordinary comparison's two `?` receiving boundaries, and exercises both
complete receivers through that two-root boundary with exact-short and zero
work: the first full Err survives and only one actual `id` call executes.
No proof or second-root evaluation can follow the first Err.

No production budget, work ceiling, physical acquisition, full-old receiver,
public diagnostic, or conditional replay policy changes. The existing
allocator/exact-small0..4/first-sticky physical controls remain unchanged.
The original full runtime log is
`/Volumes/Semaprax/lane-prompts/large-next-consolidated-4ec48-nine-20261015/owning-selected-328.log`.
This source correction does not establish runtime qualification or OPT826
closure; the complete current round and subsequent owning execution remain
required.

Actual selector348 additionally exposed a composition overlap: the weighted
sharing inverse grafted the complete old canonical receiver, then the existing
older inverse grafted that same complete receiver a second time. The correction
keeps every older inverse and original archive. It uses the older restoration
once, then checks that its complete restored receiver equals the independently
compiled full old receiver, before the unchanged whole-source archive equality.
Static byte reconstruction confirms both full equalities. The old receiver is
neither excluded nor replaced by a smaller signature-only control.

A completed bounded tool-disabled Sonnet review identified that the added local
receiving helper mirrored the production caller, and that `new()` depended on
ambient test state. The followup replaces that helper/text scan with actual
`factored::compare` execution. A scoped cfg(test)-only observer shortens the
work allowance precisely at its first real shared canonical entry and counts
actual entries. The owning control includes the real surrounding reservations
and verifies only one id() entry on the returned Err. Detached parity uses
explicit `ReplayBudget::unobserved()`. This is authored, unexecuted control
coverage; no production work/budget policy is changed.
