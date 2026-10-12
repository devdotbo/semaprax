//! Borrowed production membership against the saved owned recursive oracle.
use super::*;
const SOURCE: &str = r#"module effect.views;
permit {host.math}
@id("host.rust") interface RustHost permits {host.math} {
 @id("host.combine") import rust fn combine(left:i64,right:i64)->i64 effects {host.math} failure infallible;
}
@id("call.scalar") fn scalar(value:i64)->i64 uses {host.math} {value}
@id("call.direct") fn direct(value:i64)->i64 uses {host.math} {scalar(value)}
@id("call.native") fn native(value:i64)->i64 uses {host.math} {combine(value,value)}
@id("app.main") fn main()->i64 {0}
"#;
fn compare_expression(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let mut iterative = HirValidator::new(program).unwrap();
    let mut scope = BTreeMap::new();
    for parameter in &function.params {
        scope.insert(
            parameter.id.clone(),
            ValidationBinding {
                contract_local: false,
                mutable: false,
                ty: parameter.ty.clone(),
                ownership: parameter.ownership,
                availability: Availability::Available,
                active_loans: BTreeSet::new(),
                moved_places: BTreeMap::new(),
                definitely_partial: BTreeSet::new(),
            },
        );
        if parameter.ty == ResolvedType::SliceU8 {
            iterative.byte_slice_aliases.insert(
                parameter.id.clone(),
                Place {
                    root: parameter.id.clone(),
                    projections: Vec::new(),
                },
            );
        }
    }
    let mut recursive = iterative.clone();
    let mut recursive_scope = scope.clone();
    let effects = function.effects.iter().cloned().collect();
    let borrowed = function.effects.iter().map(String::as_str).collect();
    let reference = recursive.validate_expr_recursive_reference(
        &execution,
        expression,
        &mut recursive_scope,
        "body",
        true,
        Some(&effects),
    );
    let actual = iterative.validate_expr_iterative_effects(
        &execution,
        expression,
        &mut scope,
        "body",
        true,
        Some(Allowed::Borrowed(&borrowed)),
    );
    HirValidator::assert_validation_oracle(
        &actual,
        &reference,
        &iterative,
        &recursive,
        &scope,
        &recursive_scope,
        "body",
    );
    actual
}

#[test]
fn borrowed_effect_membership_keeps_saved_call_diagnostics_and_validation_state() {
    let program = crate::hir::resolve(
        &crate::parse(SOURCE, std::path::Path::new("effect-views.spx")).unwrap(),
    )
    .unwrap();
    crate::hir::validate(&program).unwrap();
    for id in ["call.direct", "call.native"] {
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        compare_expression(&program, function, &function.body).unwrap();
        let mut missing = function.clone();
        missing.effects.clear();
        let error = compare_expression(&program, &missing, &function.body).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("undeclared effect"));
        let mut hostile = function.body.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut hostile.kind else {
            panic!("block")
        };
        tail.ty = ResolvedType::Bool;
        assert_eq!(
            compare_expression(&program, function, &hostile)
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    let keys = [
        "process.args.read".to_owned(),
        "process.stdout.write".to_owned(),
    ];
    let owned = keys.iter().cloned().collect();
    let borrowed = keys.iter().map(String::as_str).collect();
    for op in [
        ResolvedHostCommandOperation::ArgsLen,
        ResolvedHostCommandOperation::StdoutAppend,
        ResolvedHostCommandOperation::StderrAppend,
    ] {
        assert_eq!(
            host_command::require_effects(op, Some(&owned)),
            host_command::require_effects_view(op, Some(Allowed::Borrowed(&borrowed)))
        );
        assert_eq!(
            host_command::require_effects(op, None),
            host_command::require_effects_view(op, None)
        );
    }
}
#[test]
fn repeated_effect_key_construction_removes_literal_text_payload_through_drop() {
    let keys = [
        "host.math",
        "host.math.extended",
        "λ.audit",
        "host.math",
        "audit.first",
    ]
    .map(str::to_owned);
    // Set nodes still allocate. This observes only key construction and Drop,
    // excluding original declarations and the validator's other owned metadata.
    let text_payload = "host.math".len()
        + "host.math.extended".len()
        + "λ.audit".len()
        + "host.math".len()
        + "audit.first".len();
    let (_, old) = crate::test_allocation_observer::measure(|| {
        for _ in 0..64 {
            let set: BTreeSet<String> = keys.iter().cloned().collect();
            assert_eq!(set.len(), 4);
            assert!(!set.contains("host.mat"));
            drop(set);
        }
    });
    let (_, actual) = crate::test_allocation_observer::measure(|| {
        for _ in 0..64 {
            let set: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
            assert_eq!(set.len(), 4);
            for key in &set {
                assert!(keys
                    .iter()
                    .any(|original| std::ptr::eq(*key, original.as_str())));
            }
            assert!(!set.contains("host.mat"));
            drop(set);
        }
    });
    assert!(old.allocated >= actual.allocated + 64 * text_payload);
    assert_eq!(old.allocated, old.freed);
    assert_eq!(actual.allocated, actual.freed);
    let old: BTreeSet<String> = keys.iter().cloned().collect();
    let borrowed: BTreeSet<&str> = keys.iter().map(String::as_str).collect();
    assert!(old.iter().map(String::as_str).eq(borrowed.iter().copied()));
    for probe in [
        "host.math",
        "host.math.extended",
        "λ.audit",
        "host.mat",
        "audit.first",
        "",
    ] {
        assert_eq!(
            old.contains(probe),
            Allowed::Borrowed(&borrowed).contains(probe)
        );
    }
}
