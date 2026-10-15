# Borrowed native container admission flags — source batch

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

Base:58ee1bcad7d0bf4b714ddcb76e527962f404ea17. Owning source:source_verify/type_table/native_containers.rs.

TypeTable::new previously constructed and dropped three complete Type trees only to convert them back into exactly OwnedOptionString, OwnedResultStringI64 and OwnedResultStringOptionI64. The borrowed query compares those same enum values against native imports. It leaves all source, ownership, native authority and backend verification unchanged. NATIVE-RUST-RICH-INTEROP-V1 closed constructor admission remains authoritative.

The entire original three-kind constructor is retained independently. Controls cover all current ImportResult variants, native=false and native=true, absent imports, mixed/duplicate interfaces, exact three flag values, original/current equality, actual temporary backing conservation through Drop, allocation-free lookup at zero allowance, and sticky pre-existing refusal. No measurement subtraction, raised cap, unrelated debit, or skip is introduced. The existing constructor source audit joins the new child.

Focused selectors after the complete implementation batch:

- source_verify::type_table::native_containers::tests::borrowed_flags_preserve_complete_owned_constructor_and_native_only_admission
- source_verify::type_table::native_containers::tests::borrowed_flags_remove_only_real_discarded_type_backing_through_drop
- source_verify::constructor_tests::complete_saved_pass_sources_and_all_children_remain_joined_and_within_caps

Only Rustfmt and diff whitespace validation were performed. Compilation, runtime assertions, whole-subtree funding sufficiency and application G171 recovery remain unqualified. No end-to-end token or runtime gain is claimed.
