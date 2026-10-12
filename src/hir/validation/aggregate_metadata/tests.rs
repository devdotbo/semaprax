//! Independent recursive diagnostics/state plus actual removed acquisition payloads.
use super::*;
use crate::test_allocation_observer::measure;
const SOURCE: &str = r#"module aggregate.borrowed_metadata;
@id("packet") record Packet { @id("packet.label") label:string, @id("packet.n") n:i64, }
@id("choice") variant Choice { @id("choice.a") A { @id("choice.a.n") n:i64, }, @id("choice.b") B, }
@id("construct") fn construct()->Packet { Packet { label:"label", n:7 } }
@id("replace") fn replace(input:own Packet)->Packet { input with { n:8 } }
@id("variant") fn make_choice()->Choice { Choice::A { n:9 } }
@id("match") fn inspect(input:Choice)->i64 { match input { Choice::A {n}=>n, Choice::B {}=>0, } }
@id("generic") fn make_option()->Option<i64> { Option<i64>::Some { value:1 } }
@id("main") fn main()->i64 {0}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(SOURCE, std::path::Path::new("aggregate-metadata.spx")).unwrap(),
    )
    .unwrap()
}
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
    let reference = recursive.validate_expr_recursive_reference(
        &execution,
        expression,
        &mut recursive_scope,
        "body",
        true,
        Some(&effects),
    );
    let actual = iterative.validate_expr_iterative(
        &execution,
        expression,
        &mut scope,
        "body",
        true,
        Some(&effects),
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

fn tail(expression: &mut ResolvedExpr) -> &mut ResolvedExpr {
    if let ResolvedExprKind::Block { tail, .. } = &mut expression.kind {
        tail
    } else {
        panic!("fixture keeps block body")
    }
}
#[test]
fn borrowed_aggregate_frames_preserve_recursive_diagnostics_current_metadata_and_scope() {
    let program = program();
    crate::hir::validate(&program).unwrap();
    for id in ["construct", "replace", "variant", "match", "generic"] {
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        compare_expression(&program, function, &function.body).unwrap();
        for mutation in 0..3 {
            let mut expression = function.body.clone();
            let value = tail(&mut expression);
            match (&mut value.kind, mutation) {
                (
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. }
                    | ResolvedExprKind::UpdateRecord { fields, .. },
                    0,
                ) => fields[0].field = DeclarationId::new("foreign.field"),
                (
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. }
                    | ResolvedExprKind::UpdateRecord { fields, .. },
                    1,
                ) => {
                    fields.push(fields[0].clone());
                }
                (
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. }
                    | ResolvedExprKind::UpdateRecord { fields, .. },
                    2,
                ) => fields[0].value.ty = ResolvedType::Bool,
                (ResolvedExprKind::Match { arms, .. }, 0) => arms.pop().map(drop).unwrap(),
                (ResolvedExprKind::Match { arms, .. }, 1) => {
                    arms.push(arms[0].clone());
                }
                (ResolvedExprKind::Match { arms, .. }, 2) => {
                    arms[0].value.ownership = OwnershipMode::Borrow
                }
                _ => unreachable!(),
            }
            assert!(
                compare_expression(&program, function, &expression).is_err(),
                "{id}/{mutation}"
            );
        }
    }
    // Exact current indexed metadata is borrowed, never a detached copied promise.
    let id = DeclarationId::new("packet");
    let fields = record_fields(&program, &id).unwrap();
    assert_eq!(
        fields.as_ptr(),
        program.declarations.record_fields(&id).unwrap().as_ptr()
    );
    assert!(record_fields(&program, &DeclarationId::new("missing")).is_none());
    assert!(variant_cases(&program, &id).is_none());
}

// Saved acquisition operations from the four iterative frame constructors.
// They deliberately retain the old full Vec/ID/type payload clone and Drop.
fn owned_record(
    program: &ResolvedProgram,
    id: &DeclarationId,
    args: &[ResolvedType],
) -> (
    Vec<ResolvedFieldDeclaration>,
    DeclarationId,
    Vec<ResolvedType>,
) {
    (
        program.declarations.record_fields(id).unwrap().to_vec(),
        id.clone(),
        args.to_vec(),
    )
}
fn owned_variant(
    program: &ResolvedProgram,
    id: &DeclarationId,
    args: &[ResolvedType],
) -> (
    Vec<ResolvedVariantCaseDeclaration>,
    DeclarationId,
    Vec<ResolvedType>,
) {
    (
        program.declarations.variant_cases(id).unwrap().to_vec(),
        id.clone(),
        args.to_vec(),
    )
}
#[test]
fn borrowed_aggregate_metadata_removes_actual_owned_acquisition_payload_through_drop() {
    let program = program();
    let record = DeclarationId::new("packet");
    let variant = DeclarationId::new("choice");
    let generic = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "generic")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &generic.body.kind else {
        panic!("source body stays a block")
    };
    let ResolvedType::Nominal {
        declaration: generic_id,
        arguments,
    } = &tail.ty
    else {
        panic!("actual Option instance")
    };
    assert_eq!(arguments.as_slice(), &[ResolvedType::I64]);
    // Independent shape/name census of the three actual source directories.
    let packet = record_fields(&program, &record).unwrap();
    assert_eq!(packet.len(), 2);
    assert_eq!(packet[0].name, "label");
    assert_eq!(packet[1].name, "n");
    let choice = variant_cases(&program, &variant).unwrap();
    assert_eq!(choice.len(), 2);
    assert_eq!(choice[0].name, "A");
    assert_eq!(choice[1].name, "B");
    assert_eq!(choice[0].fields.len(), 1);
    assert_eq!(choice[0].fields[0].name, "n");
    assert!(choice[1].fields.is_empty());
    let option = variant_cases(&program, generic_id).unwrap();
    assert_eq!(option.len(), 2);
    let some = option.iter().find(|case| case.name == "Some").unwrap();
    let none = option.iter().find(|case| case.name == "None").unwrap();
    assert!(none.fields.is_empty());
    assert_eq!(some.fields.len(), 1);
    assert_eq!(some.fields[0].name, "value");
    assert!(
        resolved_field_declaration_owned_capacity(&record_fields(&program, &record).unwrap()[0])
            > 0
    );
    assert!(
        resolved_variant_case_owned_capacity(&variant_cases(&program, &variant).unwrap()[0]) > 0
    );
    for count in [1, 32, 128] {
        let (_, borrowed) = measure(|| {
            for _ in 0..count {
                let fields = record_fields(&program, &record).unwrap();
                let cases = variant_cases(&program, &variant).unwrap();
                assert_eq!(fields.len(), 2);
                assert_eq!(cases.len(), 2);
                let generic_cases = variant_cases(&program, generic_id).unwrap();
                assert_eq!(
                    generic_cases.as_ptr(),
                    program
                        .declarations
                        .variant_cases(generic_id)
                        .unwrap()
                        .as_ptr()
                );
                assert_eq!(
                    fields.as_ptr(),
                    program
                        .declarations
                        .record_fields(&record)
                        .unwrap()
                        .as_ptr()
                );
                assert_eq!(
                    cases.as_ptr(),
                    program
                        .declarations
                        .variant_cases(&variant)
                        .unwrap()
                        .as_ptr()
                );
            }
        });
        assert!(!borrowed.invalid);
        assert_eq!(borrowed.allocations, 0);
        assert_eq!(borrowed.allocated, 0);
        assert_eq!(borrowed.live, 0);
        let (_, owned) = measure(|| {
            for _ in 0..count {
                let record_copy = owned_record(&program, &record, &[]);
                let variant_copy = owned_variant(&program, &variant, &[]);
                assert_eq!(
                    record_copy.0.as_slice(),
                    record_fields(&program, &record).unwrap()
                );
                assert_eq!(
                    variant_copy.0.as_slice(),
                    variant_cases(&program, &variant).unwrap()
                );
                assert_eq!(record_copy.1, record);
                assert_eq!(variant_copy.1, variant);
                assert!(record_copy.2.is_empty());
                assert!(variant_copy.2.is_empty());
                let generic_copy = owned_variant(&program, generic_id, arguments);
                assert_eq!(
                    generic_copy.0.as_slice(),
                    variant_cases(&program, generic_id).unwrap()
                );
                assert_eq!(generic_copy.1, *generic_id);
                assert_eq!(generic_copy.2.as_slice(), arguments.as_slice());
                drop((record_copy, variant_copy, generic_copy));
            }
        });
        // Packet owns two field slots; Choice and Option each own two case
        // slots and one one-field directory. All eight nonempty names clone.
        // The actual singleton Option argument Vec is the remaining type term;
        // declaration-ID clones request no text/backing. This is an independent
        // literal lower bound, not the observed total or a capacity estimator.
        let literal = count
            * (4 * std::mem::size_of::<ResolvedFieldDeclaration>()
                + 4 * std::mem::size_of::<ResolvedVariantCaseDeclaration>()
                + std::mem::size_of::<ResolvedType>()
                + "label".len()
                + "n".len()
                + "A".len()
                + "B".len()
                + "n".len()
                + "Some".len()
                + "None".len()
                + "value".len());
        assert!(!owned.invalid);
        assert!(owned.allocations >= 6 * count);
        assert!(owned.allocated >= literal);
        assert_eq!(owned.allocated, owned.freed);
        assert_eq!(owned.live, 0);
    }
}
