use super::super::*;
use super::*;

const SOURCE: &str = "module local; @id(\"local.first\") fn first() -> i64 { 1 } @id(\"local.middle\") fn middle() -> i64 { 2 } @id(\"local.last\") fn last() -> i64 { 3 }";

// Actual cold insertion API; no decoded or forged lease constructor.
pub(crate) fn cold() -> (FrontendPass, Program, CheckedModuleLease) {
    let synthetic = crate::check(SOURCE, "local.spx").unwrap();
    let resolved = crate::hir::resolve(&synthetic).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let original = resolved.functions.as_ptr();
    let mut pass = FrontendPass {
        indexed_imports: Vec::new(),
        entries: BTreeMap::from([(
            "local.spx".to_owned(),
            Arc::new(CachedModule {
                source: SOURCE.to_owned(),
                program: Arc::new(synthetic.clone()),
            }),
        )]),
        parsed: 0,
        reused: 0,
        canonicalizations: 0,
        parsed_bytes: 0,
        reused_bytes: 0,
        resolved: 0,
        retained_source_bytes: 0,
        ast_budget: 0,
        semantic: true,
        checked: BTreeMap::new(),
        next_checked: BTreeMap::new(),
        checked_reused: 0,
        functions_resolved: 0,
        functions_reused: 0,
    };
    let input = pass.resolved_module(
        "local.spx",
        synthetic.clone(),
        resolved,
        11,
        13,
        BTreeMap::new(),
        0,
    );
    let ResolvedModuleInput::Checked(owner) = input else {
        panic!("semantic cold insertion must share")
    };
    assert_eq!(original, owner.functions.as_ptr());
    pass.checked = std::mem::take(&mut pass.next_checked);
    (pass, synthetic, owner)
}

#[test]
fn cold_and_warm_checked_owner_preserves_body_pointer_and_exact_source_guards() {
    let (mut pass, synthetic, cold) = cold();
    let pointer = cold.functions.as_ptr();
    let (warm, bytes, loans) = pass.checked_module("local.spx", &synthetic).unwrap();
    assert_eq!((bytes, loans), (11, 13));
    assert_eq!(pointer, warm.functions.as_ptr());
    assert_eq!(
        crate::cache_codec::encode(&*cold).unwrap(),
        crate::cache_codec::encode(&*warm).unwrap()
    );
    let mut drift = synthetic.clone();
    drift.functions[0].name_span.end += 1;
    assert!(pass.checked_module("local.spx", &drift).is_none());
    let mut stub = synthetic.clone();
    stub.functions[0].stable_id.push_str(".forged");
    assert!(pass.checked_module("local.spx", &stub).is_none());
    drop(pass);
    drop(cold);
    crate::hir::validate(&warm).unwrap();
    assert_eq!(pointer, warm.functions.as_ptr());
    // The inverse drop order also retains the exact immutable cache owner.
    let (mut pass, synthetic, cold) = self::cold();
    drop(cold);
    let (warm, _, _) = pass.checked_module("local.spx", &synthetic).unwrap();
    drop(warm);
    assert!(pass.checked_module("local.spx", &synthetic).is_some());
}
