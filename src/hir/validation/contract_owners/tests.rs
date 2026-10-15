use crate::hir::{self, ResolvedExprKind, ResolvedProgram, ResolvedStatement};
const SOURCE: &str = r#"module contract.hir;
@id("inspect") fn inspect(input:own Bytes)->bool {byte_len(bytes_as_slice(input))==2usize}
@id("factory") fn factory(n:usize)->Bytes {bytes_zeroed(2usize)}
@id("probe") fn probe(input:own Bytes)->Bytes
 requires {let local=bytes_zeroed(2usize);inspect(local)}
 ensures {let checked=bytes_zeroed(2usize);byte_len(bytes_as_slice(checked))==2usize}
 {bytes_zeroed(1usize)}
@id("main") fn main()->i64{0}
"#;
fn program() -> ResolvedProgram {
    let ast = crate::check(SOURCE, "contract-hir.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    program
}
fn initializer(program: &mut ResolvedProgram, ensures: bool) -> &mut hir::ResolvedExpr {
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let clause = if ensures {
        &mut function.ensures[0]
    } else {
        &mut function.requires[0]
    };
    let ResolvedExprKind::Block { statements, .. } = &mut clause.kind else {
        panic!("contract block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("fresh local")
    };
    value
}
#[test]
fn forged_contract_initializer_cannot_take_preexisting_or_provisional_owner() {
    for ensures in [false, true] {
        let mut forged = program();
        let function = forged
            .functions
            .iter()
            .find(|f| f.id.as_str() == "probe")
            .unwrap();
        let root = if ensures {
            function.result_id.clone()
        } else {
            function.params[0].id.clone()
        };
        initializer(&mut forged, ensures).kind = ResolvedExprKind::Place(hir::Place {
            root,
            projections: Vec::new(),
        });
        assert!(hir::validate(&forged)
            .unwrap_err()
            .message
            .contains("contract cannot transfer ownership into a local binding"));
    }
}
#[test]
fn signature_compatible_user_factory_does_not_mint_constructor_freshness() {
    let mut forged = program();
    let factory = forged
        .functions
        .iter()
        .find(|f| f.id.as_str() == "factory")
        .unwrap()
        .id
        .clone();
    let ResolvedExprKind::Call { callee, .. } = &mut initializer(&mut forged, false).kind else {
        panic!("allocation")
    };
    *callee = factory;
    assert!(hir::validate(&forged)
        .unwrap_err()
        .message
        .contains("contract cannot transfer ownership into a local binding"));
}
#[test]
fn forged_fresh_local_identity_cannot_alias_the_result_domain() {
    let mut forged = program();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    let root = function.result_id.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut function.ensures[0].kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { binding, .. } = &mut statements[0] else {
        panic!("local")
    };
    binding.id = root;
    assert!(hir::validate(&forged)
        .unwrap_err()
        .message
        .contains("non-canonical identity"));
}

#[path = "tests/old.rs"]
mod old;
fn repeated_program(count: usize) -> ResolvedProgram {
    let clauses = (0..count)
        .map(|_| {
            " requires {let local=bytes_zeroed(2usize);byte_len(bytes_as_slice(local))==2usize}\n"
        })
        .collect::<String>();
    let source=format!("module contract.repeated;\n@id(\"probe\") fn probe(input:own Bytes)->Bytes\n{clauses}{{bytes_zeroed(1usize)}}\n@id(\"main\") fn main()->i64{{0}}\n");
    let ast = crate::check(&source, "contract-repeated.spx").unwrap();
    hir::resolve(&ast).unwrap()
}
fn outer_scope(
    function: &hir::ResolvedFunction,
) -> std::collections::BTreeMap<hir::ValueId, super::super::super::ValidationBinding> {
    function
        .params
        .iter()
        .map(|parameter| {
            (
                parameter.id.clone(),
                super::super::super::ValidationBinding {
                    contract_local: true,
                    mutable: true,
                    ty: parameter.ty.clone(),
                    ownership: parameter.ownership,
                    availability: super::super::super::Availability::Available,
                    moved_places: std::collections::BTreeMap::new(),
                    definitely_partial: std::collections::BTreeSet::new(),
                    active_loans: std::collections::BTreeSet::new(),
                },
            )
        })
        .collect()
}
#[test]
fn persistent_contract_domains_match_the_complete_saved_owning_oracle() {
    use super::super::HirValidator;
    for count in [0, 1, 2, 17] {
        let program = repeated_program(count);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "probe")
            .unwrap();
        let execution = hir::FunctionExecutionId::Monomorphic(function.id.clone());
        let outer = outer_scope(function);
        let before = outer.clone();
        let mut current = HirValidator::new(&program).unwrap();
        let mut old = current.clone();
        let expected = old.validate_contracts_owned_reference(
            &execution,
            &function.requires,
            &outer,
            "requires",
        );
        let actual = current.validate_contracts(&execution, &function.requires, &outer, "requires");
        HirValidator::assert_validation_oracle(
            &actual,
            &expected,
            &current,
            &old,
            &outer,
            &before,
            "repeated contracts",
        );
        HirValidator::assert_scope_auxiliary_oracle(&current, &old);
        actual.unwrap();
        assert_eq!(outer, before);
    }
}
#[test]
fn contract_domain_flags_and_first_error_preserve_preexisting_owner_protection() {
    use super::super::HirValidator;
    for ensures in [false, true] {
        let mut program = program();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "probe")
            .unwrap();
        let root = if ensures {
            function.result_id.clone()
        } else {
            function.params[0].id.clone()
        };
        initializer(&mut program, ensures).kind = ResolvedExprKind::Place(hir::Place {
            root: root.clone(),
            projections: Vec::new(),
        });
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "probe")
            .unwrap();
        let execution = hir::FunctionExecutionId::Monomorphic(function.id.clone());
        let mut outer = outer_scope(function);
        let mut protected = outer.values().next().unwrap().clone();
        protected.ty = function.return_type.clone();
        outer.insert(function.result_id.clone(), protected);
        let before = outer.clone();
        let clauses = if ensures {
            &function.ensures
        } else {
            &function.requires
        };
        let phase = if ensures { "ensures" } else { "requires" };
        let mut current = HirValidator::new(&program).unwrap();
        let mut old = current.clone();
        let expected = old.validate_contracts_owned_reference(&execution, clauses, &outer, phase);
        let actual = current.validate_contracts(&execution, clauses, &outer, phase);
        HirValidator::assert_validation_oracle(
            &actual, &expected, &current, &old, &outer, &before, phase,
        );
        HirValidator::assert_scope_auxiliary_oracle(&current, &old);
        assert!(actual
            .unwrap_err()
            .message
            .contains("contract cannot transfer ownership into a local binding"));
        assert_eq!(outer, before);
    }
}
