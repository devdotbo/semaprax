use super::*;
use crate::hir::ResolvedProgram;
const SOURCE: &str = r#"module replay.flat_inventory;
@id("flat.row") record Row {
 @id("flat.row.n") n:i64,
 @id("flat.row.text") text:string,
 @id("flat.row.data") data:Bytes,
}
@id("flat.identity") fn identity(value:own Row)->Row {value}
@id("app.main") fn main()->i64 {0}
"#;
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "flat-inventory.spx").unwrap()).unwrap()
}
fn identity(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "flat.identity")
        .unwrap()
}
#[test]
fn flat_record_inventory_preserves_legacy_order_at_exact_and_one_short_backing() {
    let program = program();
    let function = identity(&program);
    let legacy = build_inventory(&program, function).unwrap();
    let mut primitive = ReplayPhysicalReservation::new(usize::MAX);
    assert_eq!(
        build_inventory_with_reservation(&program, function, &mut primitive)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(
        primitive.charged(),
        0,
        "the primitive profile remains independently closed"
    );
    let mut physical = ReplayPhysicalReservation::new(usize::MAX);
    let selected =
        build_flat_record_inventory_with_reservation(&program, function, &mut physical).unwrap();
    assert_eq!(selected, legacy);
    assert!(!selected.flags.is_empty());
    for slot in &selected.slots {
        let FieldLivenessShape::Record {
            declaration,
            fields,
        } = &slot.shape
        else {
            panic!("the actual source record must preserve its field inventory")
        };
        assert_eq!(declaration.as_str(), "flat.row");
        assert_eq!(
            fields.iter().map(|f| f.field.as_str()).collect::<Vec<_>>(),
            ["flat.row.n", "flat.row.text", "flat.row.data"]
        );
        assert!(matches!(fields[0].shape, FieldLivenessShape::NoDrop));
    }
    let required = physical.charged();
    assert!(required > 0);
    let (exact, refused, used) = crate::bounded_output::with_limit_usage(required, || {
        build_flat_record_inventory_with_reservation(
            &program,
            function,
            &mut ReplayPhysicalReservation::new(required),
        )
    });
    assert!(!refused);
    assert_eq!(used, required);
    assert_eq!(exact.unwrap(), legacy);
    let mut short = ReplayPhysicalReservation::new(required - 1);
    let result = build_flat_record_inventory_with_reservation(&program, function, &mut short);
    assert_eq!(result.unwrap_err().code, "SPX-H006");
    assert!(
        short.reserve(0).is_err(),
        "a failed construction cannot reuse spare capacity"
    );
    assert!(short.charged() < required);
}
#[test]
fn malformed_flat_record_identity_layout_and_recursive_carriers_refuse_before_backing() {
    for attack in 0..6 {
        let mut program = program();
        let record = program
            .types
            .iter_mut()
            .find(|t| t.id.as_str() == "flat.row")
            .unwrap();
        let ResolvedTypeDeclarationKind::Record { fields } = &mut record.kind else {
            panic!("record")
        };
        match attack {
            0 => fields[1].index = 0,
            1 => fields[1].id = fields[0].id.clone(),
            2 => fields[1].name = fields[0].name.clone(),
            3 => {
                fields[1].ty = ResolvedType::Nominal {
                    declaration: record.id.clone(),
                    arguments: Vec::new(),
                }
            }
            4 => fields[1].ty = ResolvedType::Str,
            _ => record.name.push('x'),
        }
        let mut physical = ReplayPhysicalReservation::new(usize::MAX);
        let result = build_flat_record_inventory_with_reservation(
            &program,
            identity(&program),
            &mut physical,
        );
        assert_eq!(result.unwrap_err().code, "SPX-H006");
        assert_eq!(
            physical.charged(),
            0,
            "invalid typed descriptor must fail before clone/carrier allocation"
        );
    }
    // Record constructor support cannot authorize selected lexical replay. The
    // source/body/index/typed-skeleton closure remains independently closed.
    let program = program();
    let result = crate::cleanup_plan::validate_retained_lexical_with_reservation(
        &program,
        identity(&program),
        ReplayPhysicalReservation::new(usize::MAX),
    );
    assert_eq!(result.err().unwrap().code, "SPX-H006");
}
