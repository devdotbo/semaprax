use super::*;
const ORIGINAL: &str = include_str!("../../tests/fixtures/record-traversal-maximum.spx");
fn source(count: u32) -> String {
    ORIGINAL.replace("4096", &count.to_string())
}
fn checked(count: u32) -> ResolvedProgram {
    let ast = crate::check(&source(count), "finite-row.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let again = crate::check(&canonical, "finite-row.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&again));
    let graph = crate::graph::to_json(&ast).unwrap();
    crate::graph::verify_json(&ast, &graph).unwrap();
    crate::hir::resolve(&ast).unwrap()
}
#[test]
fn checked_one_eight_and_32_rows_rederive_exact_cumulative_events() {
    for count in [1, 8, 32] {
        let parsed = crate::parse(&source(count), "finite-row.spx").unwrap();
        let crate::ast::ExprKind::Block { statements, .. } = &parsed.functions[0].body.kind else {
            panic!("source block")
        };
        let crate::ast::Statement::While { body, .. } = &statements[2] else {
            panic!("source loop")
        };
        let crate::ast::ExprKind::Block { statements, .. } = &body.kind else {
            panic!("source rows")
        };
        let crate::ast::Statement::Let { value, .. } = &statements[0] else {
            panic!("source row")
        };
        let crate::ast::ExprKind::ConstructRecord { fields, .. } = &value.kind else {
            panic!("source record")
        };
        let byte = &fields
            .iter()
            .find(|field| field.name == "payload")
            .unwrap()
            .value;
        assert!(crate::source_verify::finite_row_allocation(&parsed, byte));
        assert!(
            !crate::source_verify::finite_row_allocation(&parsed, &(*byte).clone()),
            "a clone cannot inherit the exact source-site authorization"
        );
        let program = checked(count);
        crate::hir::validate(&program).unwrap();
        let proof = inspect(&program).unwrap();
        assert_eq!(proof.iterations, count);
        let capacity = crate::hir::analyze_byte_data_capacity(&program).unwrap();
        let summary = capacity.function("app.main").unwrap();
        assert_eq!(summary.bytes_copy_sites, count);
        assert_eq!(summary.owned_byte_payload_bytes, u64::from(count) * 3);
    }
}
#[test]
fn count_33_original_4096_and_unsealed_source_keep_t267() {
    for count in [33, 4096] {
        assert!(crate::check(&source(count), "finite-row-refused.spx")
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-T267"));
    }
    let arbitrary = "module app; @id(\"app.main\") fn main()->i64 {let mut next=0;while next<1 {let data=bytes_zeroed(3usize);next=next+1;0}0}";
    assert!(crate::check(arbitrary, "arbitrary-while.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T252"));
    for hostile in [
        source(8).replace("next = next + 1", "next = next + 2"),
        source(8).replace("while next < 8", "while next < 7"),
        source(8).replace(
            "let rows = items;",
            "let rows = items; let escaped=vec_clone_at<Row>(rows,0usize);",
        ),
        source(8).replace(
            "payload: bytes_zeroed(3usize)",
            "payload: bytes_zeroed_bounded(3usize,3usize)",
        ),
    ] {
        assert!(
            crate::check(&hostile, "finite-row-refused.spx").is_err(),
            "{hostile}"
        );
    }
}
#[test]
fn mutated_hir_trip_producer_fields_store_and_escape_lose_authority() {
    for mutation in 0..6 {
        let mut program = checked(8);
        let K::Block { statements, .. } = &mut program.functions[0].body.kind else {
            panic!("block")
        };
        let S::While {
            condition, body, ..
        } = &mut statements[2]
        else {
            panic!("while")
        };
        if mutation == 0 {
            let K::Binary { right, .. } = &mut condition.kind else {
                panic!()
            };
            right.kind = K::Int(9);
        }
        let K::Block {
            statements: rows, ..
        } = &mut body.kind
        else {
            panic!()
        };
        match mutation {
            1 => {
                let S::Let { value, .. } = &mut rows[0] else {
                    panic!()
                };
                let K::ConstructRecord { fields, .. } = &mut value.kind else {
                    panic!()
                };
                fields.swap(0, 1);
            }
            2 => {
                let S::Let { value, .. } = &mut rows[0] else {
                    panic!()
                };
                let K::ConstructRecord { fields, .. } = &mut value.kind else {
                    panic!()
                };
                let byte = fields.iter_mut().find(|f| f.value.ty == T::Bytes).unwrap();
                let K::Call { callee, .. } = &mut byte.value.kind else {
                    panic!()
                };
                *callee = crate::hir::DeclarationId::new(crate::byte_ops::COPY_ID);
            }
            3 => {
                let S::Assign { binding, .. } = &mut rows[1] else {
                    panic!()
                };
                binding.id =
                    crate::hir::ValueId::result(&crate::hir::FunctionExecutionId::Monomorphic(
                        crate::hir::DeclarationId::new("app.main"),
                    ));
            }
            4 => {
                let S::Assign { value, .. } = &mut rows[2] else {
                    panic!()
                };
                let K::Binary { right, .. } = &mut value.kind else {
                    panic!()
                };
                right.kind = K::Int(2);
            }
            5 => {
                let S::Let { value, .. } = &mut statements[3] else {
                    panic!()
                };
                value.kind = K::Call {
                    callee: crate::hir::DeclarationId::new(crate::vec_ops::CLONE_AT_ID),
                    instance: None,
                    type_arguments: vec![],
                    args: vec![],
                };
            }
            _ => {}
        }
        assert!(inspect(&program).is_none(), "mutation {mutation}");
        assert!(
            crate::hir::validate(&program).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn one_short_scratch_and_mutated_consumer_never_select_finite_capacity() {
    let program = checked(8);
    let bytes = consumers::scratch_bytes();
    let (proof, refused, used) =
        crate::bounded_output::with_limit_usage(bytes - 1, || inspect(&program));
    assert!(proof.is_none());
    assert!(refused);
    assert_eq!(used, 0);
    for id in [
        crate::vec_ops::CLONE_AT_ID,
        crate::vec_ops::CLEAR_ID,
        "foreign.row.escape",
    ] {
        let mut changed = program.clone();
        let K::Block { statements, .. } = &mut changed.functions[0].body.kind else {
            panic!()
        };
        let S::Let { value, .. } = &mut statements[8] else {
            panic!()
        };
        let K::Call { callee, .. } = &mut value.kind else {
            panic!()
        };
        *callee = crate::hir::DeclarationId::new(id);
        assert!(inspect(&changed).is_none());
        assert!(crate::hir::validate(&changed).is_err());
    }
}

#[test]
fn complete_borrowed_traversal_scratch_is_exact_and_non_candidates_are_free() {
    let program = checked(8);
    let (proof, refused, bytes) =
        crate::bounded_output::with_limit_usage(usize::MAX, || inspect(&program));
    assert!(proof.is_some());
    assert!(!refused);
    assert!(
        bytes > consumers::scratch_bytes(),
        "both simultaneously live fixed cursors are charged"
    );
    let (proof, refused, used) =
        crate::bounded_output::with_limit_usage(bytes, || inspect(&program));
    assert!(proof.is_some());
    assert!(!refused);
    assert_eq!(used, bytes);
    let (proof, refused, _) =
        crate::bounded_output::with_limit_usage(bytes - 1, || inspect(&program));
    assert!(proof.is_none());
    assert!(refused);
    let K::Block { statements, .. } = &program.functions[0].body.kind else {
        panic!()
    };
    let S::Let { value, .. } = &statements[7] else {
        panic!()
    };
    let K::Block { statements, .. } = &value.kind else {
        panic!()
    };
    let S::While {
        condition, body, ..
    } = &statements[2]
    else {
        panic!()
    };
    let (proof, refused, used) =
        crate::bounded_output::with_limit_usage(0, || resolved(&program, condition, body));
    assert!(proof.is_none());
    assert!(!refused);
    assert_eq!(used, 0);
    let ast = crate::parse(&source(8), "finite-row.spx").unwrap();
    let crate::ast::ExprKind::Block { statements, .. } = &ast.functions[0].body.kind else {
        panic!()
    };
    let crate::ast::Statement::ForBorrow { body, .. } = &statements[7] else {
        panic!()
    };
    let (proof, refused, used) = crate::bounded_output::with_limit_usage(0, || {
        crate::source_verify::finite_row_loop(&ast, body)
    });
    assert!(proof.is_none());
    assert!(!refused);
    assert_eq!(used, 0);
}
#[test]
fn freeze_parent_permissions_do_not_authorize_other_occurrences_or_mutation() {
    for mutation in 0..4 {
        let mut program = checked(8);
        let K::Block { statements, .. } = &mut program.functions[0].body.kind else {
            panic!()
        };
        let S::Let { value, .. } = &mut statements[7] else {
            panic!()
        };
        let K::Block { statements, .. } = &mut value.kind else {
            panic!()
        };
        let S::While { body, .. } = &mut statements[2] else {
            panic!()
        };
        let K::Block { statements, .. } = &mut body.kind else {
            panic!()
        };
        if mutation == 0 {
            let S::Assign { value, .. } = &mut statements[2] else {
                panic!()
            };
            let K::Binary { right, .. } = &mut value.kind else {
                panic!()
            };
            right.kind = K::Usize(2);
        } else {
            let S::Let {
                binding: discarded,
                value: authored,
                ..
            } = &mut statements[1]
            else {
                panic!()
            };
            if mutation == 3 {
                let K::Block { statements, .. } = &authored.kind else {
                    panic!()
                };
                let S::Let { value: view, .. } = &statements[0] else {
                    panic!()
                };
                let K::VecFieldRead { element, args, .. } = &view.kind else {
                    panic!()
                };
                let element = element.clone();
                let args = args.clone();
                authored.ty = element.clone();
                authored.ownership = O::Value;
                authored.kind = K::Call {
                    callee: crate::hir::DeclarationId::new(crate::vec_ops::GET_ID),
                    instance: None,
                    type_arguments: vec![element.clone()],
                    args,
                };
                discarded.ty = element;
                discarded.ownership = O::Value;
                assert!(
                    inspect(&program).is_none(),
                    "root GET must have no borrowed freeze authority"
                );
                assert!(crate::hir::validate(&program).is_err());
                continue;
            }
            let K::Block { statements, .. } = &mut authored.kind else {
                panic!()
            };
            let S::Let { value: view, .. } = &mut statements[0] else {
                panic!()
            };
            let K::VecFieldRead { args, .. } = &mut view.kind else {
                panic!()
            };
            if mutation == 1 {
                args.clear();
            } else {
                args[1].kind = K::Usize(0);
            }
        }
        assert!(inspect(&program).is_none(), "mutation {mutation}");
        assert!(crate::hir::validate(&program).is_err());
    }
}
