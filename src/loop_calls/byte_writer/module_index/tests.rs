use super::*;
const SOURCE: &str = include_str!("../../../../tests/owned_data/byte_helper_renewal.spx");
fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "module-helper-index.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
#[test]
fn module_index_reuses_typed_target_proofs_and_preserves_each_exact_site() {
    let program = program(SOURCE);
    let index = ModuleIndex::new(&program).unwrap();
    assert_eq!(
        index.proof_count, 2,
        "leaf and relay each receive one complete proof"
    );
    assert_eq!(index.sites.len(), program.functions.len());
    for function in &program.functions {
        let old = checked_bindings(&program, function).unwrap();
        assert_eq!(index.has_sites(function), !old.is_empty());
        for (at, binding) in old {
            assert_eq!(
                index.function_sites(function).unwrap().get(at),
                Some(binding)
            );
        }
    }
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("source main block")
    };
    let (binding, value) = statements
        .iter()
        .find_map(|statement| match statement {
            crate::hir::ResolvedStatement::Assign { binding, value, .. } => Some((binding, value)),
            _ => None,
        })
        .unwrap();
    assert!(index.admits(binding, value));
    assert!(
        !index.admits(binding, &value.clone()),
        "copied identities are not this immutable phase's site authority"
    );
    assert!(index.owned_capacity_bytes() > 0);
    crate::hir::validate(&program).unwrap();
    crate::cleanup_plan::validate_program(&program).unwrap();
}
#[test]
fn module_index_exact_reservation_and_one_short_fail_before_publication() {
    let program = program(SOURCE);
    let (first, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || ModuleIndex::new(&program));
    assert!(first.is_ok());
    assert!(!overflowed);
    assert!(required > 0);
    let (exact, overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || ModuleIndex::new(&program));
    assert!(exact.is_ok());
    assert!(!overflowed);
    assert_eq!(used, required);
    let (short, overflowed, _) =
        crate::bounded_output::with_limit_usage(required - 1, || ModuleIndex::new(&program));
    assert_eq!(short.unwrap_err().code, "SPX-H006");
    assert!(overflowed);
}
#[test]
fn irrelevant_modules_have_no_new_backing_and_allocating_legacy_helpers_keep_old_profiles() {
    let plain = program("module index.plain; @id(\"main\") fn main()->i64 {0}");
    let (index, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || ModuleIndex::new(&plain));
    let index = index.unwrap();
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(index.owned_capacity_bytes(), 0);
    let legacy = program(
        r#"module index.legacy;
@id("old") fn old(input:own Bytes)->Bytes { let length=byte_len(bytes_as_slice(input)); bytes_zeroed(1usize) }
@id("main") fn main()->i64 { let mut output=bytes_zeroed(1usize); output=old(output); if byte_len(bytes_as_slice(output))==1usize {1}else{0} }
"#,
    );
    let index = ModuleIndex::new(&legacy).unwrap();
    assert_eq!(index.proof_count, 1);
    assert!(legacy
        .functions
        .iter()
        .all(|function| !index.has_sites(function)));
    assert!(legacy.functions.iter().all(
        |function| function.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    ));
    crate::hir::validate(&legacy).unwrap();
}
#[test]
fn changed_contracts_cycles_and_missing_targets_cannot_reuse_prior_index_authority() {
    let mut program = program(SOURCE);
    {
        let original = ModuleIndex::new(&program).unwrap();
        assert!(original.sites.iter().any(|(_, sites)| !sites.is_empty()));
    }
    program
        .functions
        .iter_mut()
        .find(|function| function.name == "leaf")
        .unwrap()
        .requires[0]
        .kind = ResolvedExprKind::String("allocation".into());
    let changed = ModuleIndex::new(&program).unwrap();
    assert!(!changed.has_sites(
        program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
    ));
    assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
    // Parse the same authenticated source independently; prior attached plans
    // and identities cannot make a new transitive self-call nonallocating.
    let mut cyclic = self::program(SOURCE);
    let leaf = cyclic
        .functions
        .iter_mut()
        .find(|f| f.name == "leaf")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut leaf.body.kind else {
        panic!("source block")
    };
    let crate::hir::ResolvedStatement::Assign { value, .. } = &mut statements[1] else {
        panic!("source assignment")
    };
    let ResolvedExprKind::Call { callee, .. } = &mut value.kind else {
        panic!("store call")
    };
    *callee = leaf.id.clone();
    let changed = ModuleIndex::new(&cyclic).unwrap();
    assert!(cyclic.functions.iter().all(|f| !changed.has_sites(f)));
    assert_eq!(crate::hir::validate(&cyclic).unwrap_err().code, "SPX-H006");
    let mut missing = self::program(SOURCE);
    missing.functions.retain(|function| function.name != "leaf");
    let changed = ModuleIndex::new(&missing).unwrap();
    assert!(missing
        .functions
        .iter()
        .all(|function| !changed.has_sites(function)));
    assert_eq!(crate::hir::validate(&missing).unwrap_err().code, "SPX-H006");
}
