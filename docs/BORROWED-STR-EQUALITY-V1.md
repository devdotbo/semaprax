# Borrowed Str Equality v1

Audience: compiler contributors and reviewers.

Status: source implementation checkpoint for OPT #767; current-head executable
qualification is pending. This is an exact-byte comparison of two already
admitted `borrow str` operands. `==` returns true precisely when their byte
lengths and every UTF-8 byte agree; `!=` is its Boolean complement. Empty
views, embedded NUL and Unicode retain their bytes. Unicode normalization,
character folding and borrowed Str ordering are not added. Mixed Str/String
operands retain SPX-T207; Str ordering retains SPX-T208.

Operands execute once, left to right. Existing lazy Boolean evaluation,
owner-root/projection/generation validation and last-use loans remain required.
Comparison creates no owner, String clone, byte copy or runtime allocation.
Existing source/HIR validation precedes all receiving backend paths.

The interpreter compares the actual borrowed backing. Native validates both
length-delimited views before a length check and bounded memcmp (no empty
pointer dereference). Aggregate Wasm reuses its authenticated StartsWith
receiver and exact length equality: tagged owned/provider handles stay tagged,
while ordinary guest views use the existing range helper. Neither route uses
packed pointer identity as equality.

Public borrowed-text Wasm retains its existing raw wrapper ABI and validates
both parameters in authored order before target entry. Its cumulative 65536
root-byte bound and UTF-8/range status 2/1 are unchanged. An additive module
helper is emitted only when Str Eq/Ne occurs; modules without these operations
keep their existing helper and wrapper indices and bytes. Public signatures
and borrowed-return/root admission are unchanged.

Owning gates are `--test language borrowed_str_equality::` (four bodies): canonical
round-trip/graph and hostile type/mode/root/ordering/loan controls; exact-byte
interpreter/native O0/O2 with allocation counts and late postcondition failure;
aggregate Wasm tagged owners, lazy skip, exact failure and same-instance
settlement; the existing `--test owned_data owned_leaf_vec::string_slots::borrowed_and_owned_slot_views_share_nul_utf8_storage_and_expire_before_owned_calls` adds actual Vec-slot Eq/Ne and alias reads with its unchanged no-copy allocator oracle; public text distinct addresses, differing lengths, NUL, invalid
second root, cumulative exact/one-over limit and failure/reentry. The existing
ShiftSim 15-case original-source oracle remains independent of its live
`a != b` adoption; no corpus or resource ceiling is changed.
