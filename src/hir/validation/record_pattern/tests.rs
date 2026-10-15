//! Full saved record admission plus the exact removed path-buffer boundary.
use super::*;
use crate::test_allocation_observer::measure;
use std::fmt::Write as _;

fn fixture() -> (ResolvedProgram, Vec<ResolvedRecordMatchPatternField>) {
    let mut source = String::from("module record.cursor; @id(\"leaf\") record Leaf {@id(\"leaf.x\") x:i64,@id(\"leaf.y\") y:i64,} @id(\"root\") record Root {");
    for index in 0..11 {
        write!(source, "@id(\"root.f{index}\") f{index}:Leaf,").unwrap();
    }
    source.push_str(
        "} @id(\"probe\") fn probe(row:Root)->i64{row.f0.x} @id(\"app.main\") fn main()->i64{0}",
    );
    let ast = crate::check(&source, "record-cursor.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let reparsed = crate::check(&canonical, "record-cursor.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&reparsed));
    let program = crate::hir::resolve(&reparsed).unwrap();
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("probe"));
    let span = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap()
        .body
        .span;
    let fields = (0..11)
        .map(|index| ResolvedRecordMatchPatternField {
            field: DeclarationId::new(format!("root.f{index}")),
            pattern: ResolvedRecordMatchFieldPattern::Record {
                record: DeclarationId::new("leaf"),
                instance: nominal("leaf"),
                fields: ["x", "y"]
                    .into_iter()
                    .enumerate()
                    .map(|(nested, name)| ResolvedRecordMatchPatternField {
                        field: DeclarationId::new(format!("leaf.{name}")),
                        pattern: ResolvedRecordMatchFieldPattern::Binding(ResolvedBinding {
                            id: ValueId::local(
                                &owner,
                                &format!(
                                    "body.arm.0.record.field.{index}.record.field.{nested}.binding"
                                ),
                            ),
                            name: format!("{name}{index}"),
                            ty: ResolvedType::I64,
                            ownership: OwnershipMode::Value,
                            span,
                        }),
                    })
                    .collect(),
            },
        })
        .collect();
    (program, fields)
}
fn nominal(id: &str) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: vec![],
    }
}
fn run(
    validator: &mut HirValidator<'_>,
    fields: &[ResolvedRecordMatchPatternField],
    scope: &mut BTreeMap<ValueId, ValidationBinding>,
    owned: bool,
) -> Result<(), Diagnostic> {
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("probe"));
    if owned {
        validator.validate_record_match_pattern_owned_reference(
            &owner,
            &nominal("root"),
            &DeclarationId::new("root"),
            &nominal("root"),
            fields,
            scope,
            "body.arm.0.record",
            ResolvedMatchMode::Value,
        )
    } else {
        validator.validate_record_match_pattern(
            &owner,
            &nominal("root"),
            &DeclarationId::new("root"),
            &nominal("root"),
            fields,
            scope,
            path_label::Label::child("body", ".arm.0.record", [0, 0])?,
            ResolvedMatchMode::Value,
        )
    }
}
fn compare(
    program: &ResolvedProgram,
    fields: &[ResolvedRecordMatchPatternField],
) -> Result<(), Diagnostic> {
    let mut current = HirValidator::new(program).unwrap();
    let mut old = current.clone();
    let mut scope = BTreeMap::new();
    let mut old_scope = scope.clone();
    let actual = run(&mut current, fields, &mut scope, false);
    let reference = run(&mut old, fields, &mut old_scope, true);
    HirValidator::assert_validation_oracle(
        &actual,
        &reference,
        &current,
        &old,
        &scope,
        &old_scope,
        "body.arm.0.record",
    );
    actual
}

#[test]
fn borrowed_nested_record_cursor_preserves_complete_owned_admission_and_late_state() {
    let (program, fields) = fixture();
    compare(&program, &fields).unwrap();
    // Each hostile leaf is late enough that preceding sibling bindings have
    // entered the ordered scope. Compare the complete error and retained state.
    for mutation in 0..7 {
        let mut hostile = fields.clone();
        let ResolvedRecordMatchFieldPattern::Record {
            fields: children,
            instance,
            ..
        } = &mut hostile[10].pattern
        else {
            panic!("nested")
        };
        match mutation {
            0 => *instance = nominal("root"),
            1 => children[1].field = DeclarationId::new("foreign.field"),
            2 => children[1] = children[0].clone(),
            3 => {
                children.pop();
            }
            _ => {
                let ResolvedRecordMatchFieldPattern::Binding(binding) = &mut children[1].pattern
                else {
                    panic!("binding")
                };
                match mutation {
                    4 => binding.id = ValueId::new(format!("{}:", binding.id)),
                    5 => binding.ty = ResolvedType::Bool,
                    6 => binding.ownership = OwnershipMode::Borrow,
                    _ => unreachable!(),
                }
            }
        }
        assert_eq!(compare(&program, &hostile).unwrap_err().code, "SPX-H006");
    }
    let mut hostile = fields.clone();
    hostile.swap(9, 10);
    assert!(compare(&program, &hostile)
        .unwrap_err()
        .message
        .contains("binding is not canonical"));
    let mut wildcard = fields.clone();
    wildcard[9].pattern = ResolvedRecordMatchFieldPattern::Wildcard;
    compare(&program, &wildcard).unwrap();
}

#[test]
fn nested_record_cursor_removes_actual_owned_path_buffers_through_complete_drop() {
    let (program, fields) = fixture();
    let current = HirValidator::new(&program).unwrap();
    let old = current.clone();
    let (actual, borrowed) = measure(|| {
        let mut current = current;
        let mut scope = BTreeMap::new();
        let result = run(&mut current, &fields, &mut scope, false);
        drop(scope);
        drop(current);
        result
    });
    let (reference, owned) = measure(|| {
        let mut old = old;
        let mut scope = BTreeMap::new();
        let result = run(&mut old, &fields, &mut scope, true);
        drop(scope);
        drop(old);
        result
    });
    actual.unwrap();
    reference.unwrap();
    assert!(!borrowed.invalid && !owned.invalid);
    assert_eq!(borrowed.live, 0);
    assert_eq!(owned.live, 0);
    assert_eq!(borrowed.allocated, borrowed.freed);
    assert_eq!(owned.allocated, owned.freed);
    assert!(borrowed.allocated < owned.allocated);
    assert!(borrowed.allocations < owned.allocations);
}

#[test]
fn streamed_nested_binding_labels_match_decimal_unicode_and_depth_boundaries_without_a_directory() {
    for depth in [1, 2, 32, 512] {
        let indices = (0..depth)
            .map(|offset| [0, 9, 10, 99, 100, usize::MAX][offset % 6])
            .collect::<Vec<_>>();
        let root = path_label::Label::child("body:é.λ", ".arm.{}.record", [100, 0]).unwrap();
        let mut expected = root.to_string();
        for (offset, index) in indices.iter().enumerate() {
            write!(
                expected,
                ".field.{index}{}",
                if offset + 1 == depth {
                    ".binding"
                } else {
                    ".record"
                }
            )
            .unwrap();
        }
        for owner in [
            FunctionExecutionId::Monomorphic(DeclarationId::new("owner:é.λ")),
            FunctionExecutionId::Generic(FunctionInstanceId("owner:é.λ:instance".to_owned())),
        ] {
            let id = ValueId::local(&owner, &expected);
            let (matches, overflowed, used) = crate::bounded_output::with_limit_usage(0, || {
                let current = binding_path::BindingPath::new(&root, indices.iter().copied());
                id.matches_local_display(&owner, current.len().unwrap(), &current)
            });
            assert!(matches);
            assert!(!overflowed);
            assert_eq!(used, 0);
            let (_, allocation) = measure(|| {
                for _ in 0..32 {
                    let current = binding_path::BindingPath::new(&root, indices.iter().copied());
                    assert!(id.matches_local_display(&owner, current.len().unwrap(), &current));
                }
            });
            assert!(!allocation.invalid);
            assert_eq!(allocation.allocations, 0);
            assert_eq!(allocation.allocated, 0);
            assert_eq!(allocation.live, 0);
        }
    }
}

#[test]
fn owned_and_borrowed_nested_record_paths_preserve_owned_leaf_diagnostics_and_scope() {
    let source = r#"module owned.record.cursor;
@id("leaf") record Leaf {@id("leaf.x") x:Bytes,@id("leaf.y") y:i64,}
@id("root") record Root {@id("root.f0") f0:Leaf,@id("root.f1") f1:Leaf,}
@id("probe") fn probe(row:borrow Root)->i64{row.f0.y}
@id("app.main") fn main()->i64{0}
"#;
    let program =
        crate::hir::resolve(&crate::check(source, "owned-record-cursor.spx").unwrap()).unwrap();
    let (_, template) = fixture();
    let fields = template[..2].to_vec();
    for mode in [ResolvedMatchMode::Own, ResolvedMatchMode::Borrow] {
        for mutation in 0..4 {
            let mut fields = fields.clone();
            for field in &mut fields {
                let ResolvedRecordMatchFieldPattern::Record { fields, .. } = &mut field.pattern
                else {
                    panic!("nested")
                };
                let ResolvedRecordMatchFieldPattern::Binding(binding) = &mut fields[0].pattern
                else {
                    panic!("leaf")
                };
                binding.ty = ResolvedType::Bytes;
                binding.ownership = if mode == ResolvedMatchMode::Own {
                    OwnershipMode::Own
                } else {
                    OwnershipMode::Borrow
                };
            }
            let ResolvedRecordMatchFieldPattern::Record {
                fields: children, ..
            } = &mut fields[1].pattern
            else {
                panic!("nested")
            };
            match mutation {
                0 => {}
                1 => children[0].pattern = ResolvedRecordMatchFieldPattern::Wildcard,
                _ => {
                    let ResolvedRecordMatchFieldPattern::Binding(binding) =
                        &mut children[0].pattern
                    else {
                        panic!("leaf")
                    };
                    if mutation == 2 {
                        binding.ownership = OwnershipMode::Value;
                    } else {
                        binding.id = ValueId::new(format!("{}:", binding.id));
                    }
                }
            }
            let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("probe"));
            let mut current = HirValidator::new(&program).unwrap();
            let mut saved = current.clone();
            let mut scope = BTreeMap::new();
            let mut saved_scope = BTreeMap::new();
            let actual = current.validate_record_match_pattern(
                &owner,
                &nominal("root"),
                &DeclarationId::new("root"),
                &nominal("root"),
                &fields,
                &mut scope,
                "body.arm.0.record",
                mode,
            );
            let reference = saved.validate_record_match_pattern_owned_reference(
                &owner,
                &nominal("root"),
                &DeclarationId::new("root"),
                &nominal("root"),
                &fields,
                &mut saved_scope,
                "body.arm.0.record",
                mode,
            );
            HirValidator::assert_validation_oracle(
                &actual,
                &reference,
                &current,
                &saved,
                &scope,
                &saved_scope,
                "owned-record",
            );
            if mutation == 0 {
                actual.unwrap();
            } else {
                assert_eq!(actual.unwrap_err().code, "SPX-H006");
            }
        }
    }
}
