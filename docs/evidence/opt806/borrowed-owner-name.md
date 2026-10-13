# Borrowed owner/name lookup source batch

Status: source-only; executable qualification pending.
Base: `58ee1bcad`; branch `codex/borrowed-owner-name-58`.

DeclarationIndex field_id and case_id previously formed temporary tuple keys
with owner.clone() and name.to_owned(). At this base DeclarationId clone already
shares its immutable Arc backing or static carrier and allocates no bytes.
The removed discarded allocation is the name String; do not claim identity
payload allocation savings.

The private local OwnerName trait enables a safe Borrow<dyn OwnerName> view
of the unchanged `(DeclarationId, String)` key. A stack Query borrows the owner
and name; BTreeMap::get performs the existing logarithmic tree search. No scan,
duplicate index, new map schema, unsafe code, cache encoding or public lookup
signature change is introduced. Rust orphan coherence uses the local trait
object as the Borrow parameter. Compiler type checking is pending.

Comparator components are `(&DeclarationId, &str)`: the exact existing owner
Ord executes directly, followed by String/str lexicographic byte order. Source
inspection of ids.rs:176 confirms current owner Ord compares identity text.
Tests compare all four owned/query directions against complete tuple Ord, with
prefixes, embedded NUL, Unicode normalization distinctions, empty and foreign
owners, static/shared equal identities and duplicate replacements.

Independent full old field/case receiver bodies retain key construction and
Drop. Allocation controls assert literal zero new backing at active budget zero
and exact name.len old requested Layout/frees; separate ID clone controls keep
its actual zero-backing semantics. Returned values must be the identical map
references, including misses. Real resolved class/generic/variant/prelude keys
exercise the same APIs. The existing whole-source audit joins both new children.

Authored selectors (not executed):

```text
hir::declaration_index::owner_name::tests::borrowed_owner_name_preserves_hostile_tuple_order_duplicates_and_exact_references
hir::declaration_index::owner_name::tests::borrowed_owner_name_real_class_generic_variant_and_compiler_owners_keep_full_old_lookup
```

Source-only checks: rustfmt with child traversal disabled, `git diff --check`,
and touched Rust files below 1500 lines. No Cargo, rustc, build, test or allocator
observation ran. Literal-zero and asymptotic claims above describe code and
authored controls, not runtime evidence. No whole-resolver funding claim.
Graft returned no nodes and could not create its cache; exact provided source
paths and bounded references were used. RTK.md is absent in these checkouts.

Source SHA-256:

- `src/hir/declaration_index/owner_name.rs`: `368a86a61e0e7676ab6f8740b17a1ae64509f0a5470a18075a7a3739ad73e68f`
- `src/hir/declaration_index/owner_name/tests.rs`: `e89dd56f9debb39617326f79650d76fedc00fde26c347292cd6fbeb0e3698ccd`
