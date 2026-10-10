use super::*;
const SOURCE: &str = r#"module structural.selected;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#;
fn program() -> crate::hir::ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "structural.spx").unwrap()).unwrap()
}
#[test]
fn borrowed_region_prefixes_match_legacy_and_refuse_altered_runtime_order() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    let mut budget = ReplayBudget::with_physical(function, 64 * 1024 * 1024).unwrap();
    for exit in &function.cleanup_plan.exits {
        let expected = super::super::expected_exit_regions(function, exit).unwrap();
        assert_eq!(exit.leaves_regions, expected);
        assert!(exits::regions_match(function, exit, &mut budget).unwrap());
        let mut changed = exit.clone();
        changed.leaves_regions.push(CleanupRegionId(0));
        assert!(!exits::regions_match(function, &changed, &mut budget).unwrap());
    }
}
#[test]
fn borrowed_cfg_matches_legacy_and_rejects_unknown_successor_without_panicking() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    super::super::validate_reachable_acyclic_cfg(function).unwrap();
    let mut budget = ReplayBudget::with_physical(function, 64 * 1024 * 1024).unwrap();
    validate_reachable_acyclic_cfg(function, &mut budget).unwrap();
    let mut changed = function.clone();
    let entry = changed.cleanup_plan.entry.0 as usize;
    changed.cleanup_plan.blocks[entry].terminator = CleanupTerminator::Goto(EdgeId(u32::MAX));
    let error = validate_reachable_acyclic_cfg(&changed, &mut budget).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
}
#[test]
fn cfg_backing_refusal_is_sticky_before_color_or_stack_construction() {
    let program = program();
    let function = program
        .functions()
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    assert!(function.cleanup_plan.lexical_control.is_some());
    let mut budget = ReplayBudget {
        physical: Some(Box::new(PhysicalReservation::new(0))),
        ..ReplayBudget::new()
    };
    assert_eq!(
        validate_reachable_acyclic_cfg(function, &mut budget)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(
        validate_reachable_acyclic_cfg(function, &mut budget)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert!(budget.physical.as_deref_mut().unwrap().reserve(0).is_err());
    assert_eq!(budget.physical.as_ref().unwrap().charged(), 0);
}
