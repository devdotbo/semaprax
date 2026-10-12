//! Complete receiver state and independent original canonical products.
use super::*;
use crate::hir::closure::{closure_function, inventory};
const SOURCE: &str = r#"
module test.closures;
@id("closure.helper") fn helper(value:i64)->i64{value+1}
@id("closure.main") fn main()->i64{
    let mut left=2;
    let right=3;
    let callback=fn(value:i64)->i64{value+left+right};
    left=100;
    callback(37)
}
"#;

fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(SOURCE, "closures.spx").unwrap()).unwrap()
}
fn scope(site: &ResolvedExpr) -> BTreeMap<ValueId, ValidationBinding> {
    let ResolvedExprKind::Closure { captures, .. } = &site.kind else {
        panic!("closure")
    };
    captures
        .iter()
        .map(|capture| {
            let ResolvedExprKind::Place(place) = &capture.value.kind else {
                panic!("capture")
            };
            (
                place.root.clone(),
                ValidationBinding {
                    mutable: true,
                    contract_local: true,
                    ty: capture.value.ty.clone(),
                    ownership: capture.value.ownership,
                    availability: Availability::Available,
                    active_loans: BTreeSet::new(),
                    moved_places: BTreeMap::new(),
                    definitely_partial: BTreeSet::new(),
                },
            )
        })
        .collect()
}
fn compare(program: &ResolvedProgram, site: &ResolvedExpr, mode: usize) {
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("closure.main"));
    let path =
        crate::hir::ids::checked_scoped_path(site.id.as_str(), &owner, "expression").unwrap();
    let mut current = HirValidator::new(program).unwrap();
    let mut current_scope = scope(site);
    match mode {
        1 => {
            let key = current_scope.keys().next().unwrap().clone();
            current_scope.remove(&key);
        }
        2 => {
            current_scope.values_mut().next().unwrap().availability = Availability::Moved;
        }
        3 => {
            let ResolvedExprKind::Closure { captures, .. } = &site.kind else {
                panic!("closure")
            };
            current.expression_ids.insert(captures[0].value.id.clone());
        }
        4 | 5 => {
            let mut accepted = current.clone();
            let mut accepted_scope = current_scope.clone();
            accepted
                .validate_closure_owned_reference(&owner, site, &mut accepted_scope, path, true)
                .unwrap();
            if mode == 4 {
                current
                    .expression_ids
                    .insert(accepted.expression_ids.iter().next_back().unwrap().clone());
            } else {
                current
                    .value_ids
                    .insert(accepted.value_ids.iter().next_back().unwrap().clone());
            }
        }
        _ => {}
    }
    let mut old = current.clone();
    let mut old_scope = current_scope.clone();
    let expected = old.validate_closure_owned_reference(&owner, site, &mut old_scope, path, true);
    let actual = current.validate_closure(&owner, site, &mut current_scope, path, true);
    assert_eq!(actual, expected);
    assert_eq!(current.expression_ids, old.expression_ids);
    assert_eq!(current.value_ids, old.value_ids);
    assert_eq!(current_scope, old_scope);
    assert_eq!(current.byte_slice_aliases, old.byte_slice_aliases);
    assert_eq!(current.borrowed_str_aliases, old.borrowed_str_aliases);
    assert_eq!(current.buffer_reopen_sites, old.buffer_reopen_sites);
    for allow_moves in [false, true] {
        let expected =
            old.finish_affine_invocation_owned_reference(site, &mut old_scope, allow_moves);
        let actual = current.finish_affine_invocation(site, &mut current_scope, allow_moves);
        assert_eq!(actual, expected);
        assert_eq!(current_scope, old_scope);
    }
}
#[test]
fn complete_closure_receiver_preserves_success_first_diagnostic_and_full_merge_prefix() {
    let program = program();
    let site = inventory(&program)[0];
    for mode in 0..6 {
        compare(&program, site, mode);
    }
    let expected = closure_function(&program, site).unwrap();
    let actual = crate::hir::closure::paid::function(&program, site).unwrap();
    assert_eq!(actual, expected);
}
#[test]
fn original_closure_corpus_and_hostile_products_keep_full_diagnostics() {
    let program = program();
    let original = inventory(&program)[0].clone();
    for mode in 0..5 {
        let mut site = original.clone();
        let ResolvedExprKind::Closure { captures, body, .. } = &mut site.kind else {
            panic!("closure")
        };
        match mode {
            0 => captures[0].binding.ty = ResolvedType::Bool,
            1 => captures.reverse(),
            2 => body.kind = ResolvedExprKind::String("excluded nested payload".to_owned()),
            3 => {
                let mut nested = (**body).clone();
                nested.kind = ResolvedExprKind::String("nested excluded payload".to_owned());
                body.kind = ResolvedExprKind::Block {
                    statements: Vec::new(),
                    tail: Box::new(nested),
                };
            }
            _ => captures[0].value.ownership = OwnershipMode::Own,
        }
        assert_eq!(
            crate::hir::closure::paid::function(&program, &site),
            closure_function(&program, &site)
        );
        assert!(closure_function(&program, &site).is_err());
        compare(&program, &site, 0);
    }
    // Keep this original product source itself independent of the paid receiver.
    assert!(!program.functions.is_empty());
}

const GENERIC_SOURCE: &str = r#"
module fixture.app;
@id("fixture.fill") fn fill<T>(values:own Vec<T>,replacement:T)->Vec<T>{
    let length=vec_len<T>(values);
    let mut output=vec_with_capacity<T>(length);
    let mut index=0usize;
    while index<length {
        let callback=fn(item:T)->T{replacement};
        output=vec_push<T>(output,callback(vec_get<T>(values,index)));
        index=index+1usize;
        index<length
    }
    output
}
@id("fixture.main") fn main()->i64 {0}
@id("fixture.public") fn published()->i64 {0}
"#;
#[test]
fn full_generic_closure_receiver_preserves_original_template_and_binding_collisions() {
    let program =
        crate::hir::resolve(&crate::check(GENERIC_SOURCE, "generic-closure.spx").unwrap()).unwrap();
    let template = program
        .function_templates
        .iter()
        .find(|template| template.id.as_str() == "fixture.fill")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &template.body.kind else {
        panic!("template block")
    };
    let loop_body = statements
        .iter()
        .find_map(|statement| {
            if let ResolvedStatement::While { body, .. } = statement {
                Some(body.as_ref())
            } else {
                None
            }
        })
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &loop_body.kind else {
        panic!("loop block")
    };
    let ResolvedStatement::Let { value: site, .. } = &statements[0] else {
        panic!("closure binding")
    };
    let execution = FunctionExecutionId::Monomorphic(template.id.clone());
    let path =
        crate::hir::ids::checked_scoped_path(site.id.as_str(), &execution, "expression").unwrap();
    let ResolvedExprKind::Closure {
        parameters,
        captures,
        ..
    } = &site.kind
    else {
        panic!("closure")
    };
    for mode in 0..3 {
        let mut current = HirValidator::new(&program).unwrap();
        let mut entries = template
            .params
            .iter()
            .map(|param| (param.id.clone(), &param.ty))
            .collect::<BTreeMap<_, _>>();
        if mode == 1 {
            let ResolvedExprKind::Place(place) = &captures[0].value.kind else {
                panic!("capture place")
            };
            entries.remove(&place.root);
        }
        if mode == 2 {
            current.value_ids.insert(parameters[0].id.clone());
        }
        let mut old = current.clone();
        let mut current_values = generic_template::values::Values::LegacyBorrowed(entries.clone());
        let mut old_values = generic_template::values::Values::LegacyBorrowed(entries.clone());
        let expected = old.validate_template_closure_owned_reference(
            template,
            &execution,
            site,
            &mut old_values,
            path,
        );
        let actual = current.validate_template_closure(
            template,
            &execution,
            site,
            &mut current_values,
            path,
        );
        assert_eq!(actual, expected);
        assert_eq!(current.expression_ids, old.expression_ids);
        assert_eq!(current.value_ids, old.value_ids);
        for (id, ty) in &entries {
            assert_eq!(current_values.get(id), Some(*ty));
            assert_eq!(old_values.get(id), Some(*ty));
        }
    }
}

#[test]
fn unconstrained_hostile_metadata_census_fallback_preserves_full_old_product_priority() {
    let program = program();
    let mut site = inventory(&program)[0].clone();
    let mut element = ResolvedType::I64;
    for _ in 0..600 {
        element = ResolvedType::Function {
            parameters: Vec::new(),
            result: Box::new(element),
        };
    }
    let ResolvedExprKind::Closure { body, .. } = &mut site.kind else {
        panic!("closure")
    };
    let tail = body.clone();
    body.kind = ResolvedExprKind::VecConstructor {
        element,
        count: 0,
        statements: Vec::new(),
        tail,
    };
    crate::hir::closure::validate_shape(&program, &site).unwrap();
    assert_eq!(
        crate::hir::closure::paid::function(&program, &site),
        closure_function(&program, &site)
    );
    compare(&program, &site, 0);
}
