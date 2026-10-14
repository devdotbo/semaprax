use super::*;
use crate::test_allocation_observer::measure;
fn fixture() -> ResolvedProgram {
    let source = crate::check(
        r#"module loan.storage;
@id("packet") record Packet { @id("payload") payload:Bytes, @id("sibling") sibling:Bytes, }
@id("first") fn first(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.payload);
 byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)
}
@id("second") fn second(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.sibling); byte_len(view)+byte_len(view)
}
@id("identity") fn identity<T>(value:T)->T {value}
@id("main") fn main()->i64 {identity<i64>(0)}
"#,
        "canonical-loan-storage.spx",
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap()
}
#[test]
fn current_shared_owned_and_unobserved_custody_keep_complete_original_ids_liveness_origins() {
    let program = fixture();
    let expected = Inventory::rebuild(&program).unwrap();
    let (current, refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let seed = constructor_seed::ConstructorSeed::authenticate(&program, None).unwrap();
        Lookup::current(&seed).unwrap()
    });
    assert!(!refused);
    assert!(matches!(current, Lookup::Owned(_)));
    for (key, id) in &expected.ids {
        assert_eq!(current.id(key), Some(id));
    }
    for (key, index) in &expected.liveness {
        assert_eq!(current.live(key), Some(index));
        assert_eq!(current.origin(*index), expected.origins.get(*index));
    }
    let seed = constructor_seed::ConstructorSeed::authenticate(&program, None).unwrap();
    let unobserved = Lookup::current(&seed).unwrap();
    assert!(matches!(unobserved, Lookup::UnfundedShared(..)));
    for owner in [&current, &unobserved] {
        let ((same, metrics), refused, paid) = crate::bounded_output::with_limit_usage(0, || {
            measure(|| {
                let cloned = owner.clone();
                let shared = owner.share().unwrap();
                let same = cloned.is_current(&program) && shared.is_current(&program);
                drop(cloned);
                drop(shared);
                same
            })
        });
        assert!(same && !refused && !metrics.invalid);
        assert_eq!(paid, 0);
        assert_eq!(metrics.allocated, 0);
        assert_eq!(metrics.live, 0);
    }
    let legacy = Lookup::Legacy(expected);
    let (((), metrics), _, _) =
        crate::bounded_output::with_limit_usage(usize::MAX, || measure(|| drop(legacy.clone())));
    assert!(metrics.allocated > 0 && !metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}
#[test]
fn complete_active_compatibility_constructor_is_paid_through_drop_exact_short_zero_and_sticky() {
    let program = fixture();
    let observe = |limit| {
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let result = HirValidator::new(&program);
                let ok = result.is_ok();
                drop(result);
                if !ok {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                ok
            })
        })
    };
    let ((ok, metrics), refused, paid) = observe(usize::MAX);
    assert!(ok && !refused && !metrics.invalid);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    let ((ok, metrics), refused, used) = observe(paid);
    assert!(ok && !refused && !metrics.invalid);
    assert_eq!(used, paid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    for limit in [0, paid - 1] {
        let ((ok, metrics), refused, used) = observe(limit);
        assert!(!ok && refused && used <= limit && !metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}
#[test]
fn original_compatibility_constructor_and_current_seed_order_keep_full_diagnostics_and_transplant_refusal(
) {
    let program = fixture();
    let old = HirValidator::original_with_directory(&program, None).unwrap();
    assert!(matches!(old.canonical_loans, Lookup::Legacy(_)));
    let current = HirValidator::new(&program).unwrap();
    assert!(matches!(
        current.canonical_loans,
        Lookup::UnfundedShared(..)
    ));
    let other = program.clone();
    let old = HirValidator::original_with_profiles(
        &other,
        None,
        None,
        current.executable_profiles.clone(),
    )
    .err()
    .unwrap();
    let now = HirValidator::with_profiles(&other, None, None, current.executable_profiles.clone())
        .err()
        .unwrap();
    assert_eq!(now, old);
    let mismatch = HirValidator::with_profiles(
        &other,
        None,
        current.canonical_loans.share(),
        current.executable_profiles.clone(),
    )
    .err()
    .unwrap();
    assert_eq!(
        mismatch,
        hir_error("canonical shared-loan storage does not belong to the current program")
    );
    let mut bad = program.clone();
    bad.entrypoint = DeclarationId::new("bad\0identity");
    assert_eq!(
        HirValidator::new(&bad).err().unwrap(),
        HirValidator::original_with_directory(&bad, None)
            .err()
            .unwrap()
    );
}
#[test]
fn attached_loan_drift_is_not_new_constructor_authority_and_whole_saved_receiver_remains_complete()
{
    let mut program = fixture();
    let function = program
        .functions
        .iter_mut()
        .find(|f| !f.loan_plan.loans.is_empty())
        .unwrap();
    function.loan_plan.loans[0].origin.projections.clear();
    let old = HirValidator::original_with_directory(&program, None)
        .unwrap()
        .validate();
    let current = crate::bounded_output::with_limit_usage(usize::MAX, || {
        HirValidator::new(&program).unwrap().validate()
    })
    .0;
    assert!(old.is_err());
    assert_eq!(current, old);
}

#[test]
fn complete_old_constructor_inventory_caller_and_closure_source_projections_remain_exact() {
    let current = include_str!("../canonical_loans.rs");
    let original=include_str!("../../../../docs/evidence/compatibility-loan-custody/src__hir__validation__canonical_loans.rs.before");
    let marker = "// The ordinary compatibility constructor retains the saved owned directories.";
    assert_eq!(
        current.split_once(marker).unwrap().0,
        original.split_once(marker).unwrap().0
    );
    let current = include_str!("../../validation.rs");
    let projected=current.replacen("let canonical_loans = canonical_loans::Lookup::current(&seed)?;\n        Self::from_constructor(seed, canonical_loans, None)","let canonical_loans = canonical_loans::Inventory::rebuild(program)?;\n        Self::from_constructor(seed, canonical_loans::Lookup::Legacy(canonical_loans), None)",1)
        .replacen("storage: Option<canonical_loans::Lookup<'a>>", "storage: Option<&'a super::canonical_loan_storage::Storage<'a>>",1)
        .replacen("            storage\n        } else {\n            canonical_loans::Lookup::current(&seed)?", "            canonical_loans::Lookup::Borrowed(storage)\n        } else {\n            canonical_loans::Lookup::Legacy(canonical_loans::Inventory::rebuild(program)?)",1);
    assert_eq!(
        projected,
        include_str!(
            "../../../../docs/evidence/compatibility-loan-custody/src__hir__validation.rs.before"
        )
    );
    assert_eq!(include_str!("../closure.rs").replacen("self.canonical_loans.share()", "self.canonical_loans.shared()",1),
        include_str!("../../../../docs/evidence/compatibility-loan-custody/src__hir__validation__closure.rs.before"));
    let old = include_str!("constructors_before.rs");
    let directory = old
        .split_once("    #[cfg(test)]\n")
        .unwrap()
        .1
        .split_once("    pub(super) fn original_with_profiles(")
        .unwrap()
        .0;
    let directory = "    #[cfg(test)]\n".to_owned() + directory;
    let directory = directory.replacen("original_with_directory(", "with_directory(", 1);
    let archive = include_str!(
        "../../../../docs/evidence/compatibility-loan-custody/src__hir__validation.rs.before"
    );
    let start = archive
        .find("    #[cfg(test)]\n    pub(super) fn with_directory(")
        .unwrap();
    let end = archive[start..]
        .find("    pub(in crate::hir) fn with_storage(")
        .unwrap()
        + start;
    assert_eq!(directory, &archive[start..end]);
    let saved = include_str!("constructors_before.rs");
    let (_, tail) = saved
        .split_once("    pub(super) fn original_with_profiles(")
        .unwrap();
    let profiles = "    fn with_profiles(".to_owned() + tail.strip_suffix("}\n").unwrap();
    let start = archive.find("    fn with_profiles(").unwrap();
    let end = archive[start..].find("    fn from_constructor(").unwrap() + start;
    assert_eq!(profiles, &archive[start..end]);
    let old=include_str!("../../canonical_loan_storage/caller/owned_before.rs").strip_prefix("//! Complete original paid preparation receiver, independently compiled.\nuse super::*;\n").unwrap().replacen("pub(super) fn prepare_original<'a>(","fn prepare_with_comparison<'a>(",1);
    let archive=include_str!("../../../../docs/evidence/compatibility-loan-custody/src__hir__canonical_loan_storage__caller.rs.before");
    let start = archive.find("fn prepare_with_comparison<'a>(").unwrap();
    let end = archive[start..].find("\nfn append(").unwrap() + start;
    assert_eq!(old, &archive[start..end]);
}
