//! Checked value retention limits and exact nominal-fact reuse.
use super::*;
use checked_value_retention::{retain_checked_nominal_type, retain_checked_value_types};
fn checked_value_fixture() -> hir::ResolvedProgram {
    let program = crate::parse(r#"module values;
@id("values.config") record Config { @id("values.config.value") value: i64, }
@id("values.envelope") record Envelope { @id("values.envelope.item") item: Config, }
@id("values.choice") variant Choice { @id("values.choice.some") Some { @id("values.choice.item") item: i64, }, @id("values.choice.none") None, }
@id("values.disconnected") fn disconnected() -> i64
    requires (Config { value: 1 }).value == 1
    ensures result >= 0
{
    let envelope = Envelope { item: Config { value: 42 } };
    match envelope { Envelope { item: Config { value: selected } } => selected, }
}
@id("values.variant") fn variant_value() -> i64 {
    let choice = Choice::Some { item: 42 };
    match choice { Choice::Some { item: selected } => selected, Choice::None {} => 0, }
}
@id("values.main") fn main() -> i64 { 0 }
"#, Path::new("values.spx")).unwrap();
    hir::resolve(&program).unwrap()
}
#[test]
fn nominal_values_outside_signatures_retain_exact_checked_type_facts() {
    let resolved = checked_value_fixture();
    assert!(resolved.functions.iter().all(
        |function| function.params.is_empty() && function.return_type == hir::ResolvedType::I64
    ));
    let retained =
        retained_signature_type_facts(&resolved.functions, &resolved.declarations).unwrap();
    assert_eq!(retained.len(), 3);
    for (id, kind) in [
        ("values.config", hir::DeclarationKind::Record),
        ("values.envelope", hir::DeclarationKind::Record),
        ("values.choice", hir::DeclarationKind::Variant),
    ] {
        let ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new(id.to_owned()),
            arguments: Vec::new(),
        };
        assert_eq!(
            retained.get(&ty.identity_key()),
            Some(&(kind, resolved.declarations.type_facts(&ty).unwrap()))
        );
    }
}
#[test]
fn repeated_zero_argument_nominal_facts_reuse_the_exact_retained_entry() {
    let resolved = checked_value_fixture();
    let ty = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("values.config"),
        arguments: Vec::new(),
    };
    let mut retained = BTreeMap::new();
    retain_checked_nominal_type(&ty, &resolved.declarations, &mut retained).unwrap();
    let key = ty.identity_key();
    let original_key = retained.get_key_value(&key).unwrap().0.as_ptr();
    let original_layout = retained[&key].1.layout_key.as_ptr();
    for _ in 0..128 {
        retain_checked_nominal_type(&ty, &resolved.declarations, &mut retained).unwrap();
    }
    assert_eq!(retained.len(), 1);
    assert_eq!(
        retained.get_key_value(&key).unwrap().0.as_ptr(),
        original_key
    );
    assert_eq!(retained[&key].1.layout_key.as_ptr(), original_layout);
    assert_eq!(
        retained[&key].1,
        resolved.declarations.type_facts(&ty).unwrap()
    );
}
#[test]
fn retained_value_facts_keep_the_4096_cap_and_reject_missing_checked_facts() {
    let resolved = checked_value_fixture();
    let ty = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("values.config".to_owned()),
        arguments: Vec::new(),
    };
    let facts = resolved.declarations.type_facts(&ty).unwrap();
    let mut retained = (0..MAX_DECLARATIONS)
        .map(|index| {
            (
                format!("occupied.{index}"),
                (hir::DeclarationKind::Record, facts.clone()),
            )
        })
        .collect();
    let error =
        retain_checked_nominal_type(&ty, &resolved.declarations, &mut retained).unwrap_err();
    assert!(is_named_limit(&error, "declarations"));
    assert_eq!(retained.len(), MAX_DECLARATIONS);
    let absent = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("values.absent".to_owned()),
        arguments: Vec::new(),
    };
    let error = retain_checked_nominal_type(&absent, &resolved.declarations, &mut BTreeMap::new())
        .unwrap_err();
    assert!(error.iter().any(|error| error.code == "SPX-G173"));
}
#[test]
fn checked_value_cursor_traversal_enforces_visit_and_depth_limits() {
    let resolved = checked_value_fixture();
    let mut visits = MAX_CHECKED_VALUE_VISITS - 1;
    let mut retained = BTreeMap::new();
    retain_checked_value_types(
        CheckedValueNode::Type(&hir::ResolvedType::I64),
        &resolved.declarations,
        &mut retained,
        &mut visits,
    )
    .unwrap();
    assert_eq!(visits, MAX_CHECKED_VALUE_VISITS);
    let error = retain_checked_value_types(
        CheckedValueNode::Type(&hir::ResolvedType::I64),
        &resolved.declarations,
        &mut retained,
        &mut visits,
    )
    .unwrap_err();
    assert!(is_named_limit(&error, "checked_value_visits"));
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "values.main")
        .unwrap();
    let owner = hir::FunctionExecutionId::Monomorphic(function.id.clone());
    let mut expression = function.body.clone();
    for index in 0..=MAX_CHECKED_VALUE_DEPTH {
        expression = hir::ResolvedExpr {
            id: hir::ExpressionId::new(&owner, &format!("depth.{index}")),
            ty: hir::ResolvedType::I64,
            ownership: hir::OwnershipMode::Value,
            kind: hir::ResolvedExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(expression),
            },
            span: Span::default(),
        };
    }
    let error = retain_checked_value_types(
        CheckedValueNode::Expression(&expression),
        &resolved.declarations,
        &mut retained,
        &mut 0,
    )
    .unwrap_err();
    assert!(is_named_limit(&error, "checked_value_depth"));
}
