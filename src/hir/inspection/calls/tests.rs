//! Saved preorder and owning staging oracle; receiver access scope only.
use super::*;

fn saved_visit(
    expression: &ResolvedExpr,
    visit: &mut impl FnMut(&DeclarationId, Option<&FunctionInstanceId>, &[ResolvedType]),
) {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            saved_visit(source, visit);
            saved_visit(start, visit);
            saved_visit(end, visit);
        }
        ResolvedExprKind::Closure { body, captures, .. } => {
            saved_visit(body, visit);
            for capture in captures {
                saved_visit(&capture.value, visit);
            }
        }
        ResolvedExprKind::FunctionReference { target } => visit(target, None, &[]),
        ResolvedExprKind::Invoke { callable, args } => {
            saved_visit(callable, visit);
            for arg in args {
                saved_visit(arg, visit);
            }
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } => {
            visit(callee, instance.as_ref(), type_arguments);
            for arg in args {
                saved_visit(arg, visit);
            }
        }
        ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => {
            for arg in args {
                saved_visit(arg, visit);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for arg in &call.args {
                saved_visit(arg, visit);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for arg in &call.args {
                saved_visit(arg, visit);
            }
        }
        ResolvedExprKind::LexicalExit { value, .. } => {
            if let Some(value) = value {
                saved_visit(value, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => saved_visit(value, visit),
        ResolvedExprKind::Binary { left, right, .. } => {
            saved_visit(left, visit);
            saved_visit(right, visit);
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        saved_visit(child, visit);
                    }
                }
            }
            saved_visit(tail, visit);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            saved_visit(condition, visit);
            saved_visit(then_branch, visit);
            saved_visit(else_branch, visit);
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                saved_visit(&field.value, visit);
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                saved_visit(&field.value, visit);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            saved_visit(scrutinee, visit);
            for arm in arms {
                // A guard is authored before the arm value and, under left to
                // right evaluation, runs before it. Skipping it would hide a
                // callee reached only from a guard.
                if let Some(guard) = &arm.guard {
                    saved_visit(guard, visit);
                }
                saved_visit(&arm.value, visit);
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            saved_visit(base, visit);
            for field in fields {
                saved_visit(&field.value, visit);
            }
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::BorrowedUtf8Literal(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
}

fn resolved(source: &str) -> ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(source, std::path::Path::new("lifecycle-stream.spx")).unwrap(),
    )
    .unwrap()
}
#[test]
fn borrowed_call_stream_matches_saved_preorder_short_circuit_and_removes_wide_staging() {
    let mut source = String::from("module stream; @id(\"stream.target\") fn target()->i64 {1} @id(\"stream.main\") fn main()->i64 {");
    for index in 0..96 {
        source.push_str(&format!("let value{index}=target();"));
    }
    source.push_str("0}");
    let program = resolved(&source);
    let root = &program.functions[1].body;
    let before = crate::cache_codec::encode(&program).unwrap();
    let mut saved = Vec::new();
    saved_visit(root, &mut |id, instance, arguments| {
        saved.push((id.clone(), instance.cloned(), arguments.to_vec()))
    });
    assert_eq!(saved.len(), 96);
    assert!(saved
        .iter()
        .all(|(id, instance, arguments)| id.as_str() == "stream.target"
            && instance.is_none()
            && arguments.is_empty()));
    let literal_payload =
        96 * std::mem::size_of::<(DeclarationId, Option<FunctionInstanceId>, Vec<ResolvedType>)>();
    let (_, old) = crate::test_allocation_observer::measure(|| {
        let mut staged = Vec::new();
        saved_visit(root, &mut |id, instance, arguments| {
            staged.push((id.clone(), instance.cloned(), arguments.to_vec()))
        });
        assert_eq!(staged, saved);
        drop(staged);
    });
    let (_, current) = crate::test_allocation_observer::measure(|| {
        let mut count = 0;
        try_visit_resolved_calls(root, &mut |id, instance, arguments| {
            assert_eq!(id, &saved[count].0);
            assert_eq!(instance, saved[count].1.as_ref());
            assert_eq!(arguments, saved[count].2.as_slice());
            let target = program.resolve_call_target(id, instance).unwrap();
            let mut effects = BTreeSet::new();
            extend_resolved_lifecycle_effects(&program, &target.return_type, &mut effects).unwrap();
            assert!(effects.is_empty());
            count += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
        assert_eq!(count, 96);
    });
    assert!(old.allocated >= literal_payload);
    assert_eq!(
        (
            current.allocations,
            current.reallocations,
            current.allocated,
            current.live
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(old.live, 0);
    assert!(!old.invalid && !current.invalid);
    for stop in [0usize, 1, 48, 95] {
        let mut visited = 0;
        let result = try_visit_resolved_calls(root, &mut |id, _, _| {
            assert_eq!(id, &saved[visited].0);
            let at = visited;
            visited += 1;
            if at == stop {
                Err(at)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(stop));
        assert_eq!(visited, stop + 1);
    }
    crate::hir::validate(&program).unwrap();
    assert_eq!(crate::cache_codec::encode(&program).unwrap(), before);
}
#[test]
fn closure_body_precedes_captures_and_function_references_keep_saved_call_order() {
    let program = resolved(
        r#"module stream.closure;
@id("call.zed") fn zed(value:i64)->i64 {value}
@id("call.alpha") fn alpha(value:i64)->i64 {value}
@id("call.main") fn main()->i64 {
 let offset=2; let callback=fn(value:i64)->i64 {zed(value)+offset}; callback(3)
}
"#,
    );
    let mut body = program.functions[2].body.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("closure let")
    };
    let ResolvedExprKind::Closure { captures, .. } = &mut value.kind else {
        panic!("closure")
    };
    assert_eq!(captures.len(), 1);
    // Hostile shape is inspection data only: it grants no closure/HIR authority.
    captures[0].value.kind = ResolvedExprKind::FunctionReference {
        target: program.functions[1].id.clone(),
    };
    let mut old = Vec::new();
    saved_visit(&body, &mut |id, _, _| old.push(id.clone()));
    assert_eq!(
        old.iter().map(DeclarationId::as_str).collect::<Vec<_>>(),
        ["call.zed", "call.alpha"]
    );
    let mut current = Vec::new();
    try_visit_resolved_calls(&body, &mut |id, _, _| {
        current.push(id.clone());
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(current, old);
    let mut visited = 0;
    assert_eq!(
        try_visit_resolved_calls(&body, &mut |_, _, _| {
            visited += 1;
            Err(7u8)
        }),
        Err(7)
    );
    assert_eq!(visited, 1);
}

fn saved_effects(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<BTreeSet<String>, Diagnostic> {
    fn collect(
        program: &ResolvedProgram,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<DeclarationId>,
        effects: &mut BTreeSet<String>,
    ) -> Result<(), Diagnostic> {
        let Some(id) = ty.nominal_id() else {
            return Ok(());
        };
        if !visiting.insert(id.clone()) {
            return Ok(());
        }
        let declaration = program
            .types
            .iter()
            .find(|item| item.id == *id)
            .ok_or_else(|| hir_error(format!("type `{id}` has no lifecycle declaration")))?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                if let ResolvedResourceDropKind::Imported { import, .. } = &drop.kind {
                    let resolved = program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|item| item.id == *import)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "resource `{id}` references missing import `{import}`"
                            ))
                        })?;
                    effects.extend(resolved.effects.iter().cloned());
                }
            }
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    collect(program, &field.ty, visiting, effects)?;
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        collect(program, &field.ty, visiting, effects)?;
                    }
                }
            }
        }
        visiting.remove(id);
        Ok(())
    }

    let mut effects = BTreeSet::new();
    collect(program, ty, &mut BTreeSet::new(), &mut effects)?;
    Ok(effects)
}

#[test]
fn lifecycle_union_keeps_saved_sorted_effects_missing_type_diagnostics_and_validator_priority() {
    let program = resolved(
        r#"module stream.lifecycle;
permit {filesystem.handle.release, filesystem.audit.record}
@id("io.file") resource File { @id("io.file.drop") drop import "io.file.finalize"; }
@id("io.host") interface Host permits {filesystem.handle.release, filesystem.audit.record} {
 @id("io.file.finalize") import fn finalize(file:own File)->unit
 effects {filesystem.handle.release, filesystem.audit.record} failure infallible consumes file always;
}
@id("io.consume") fn consume(file:own File)->i64 uses {filesystem.handle.release, filesystem.audit.record} {0}
@id("app.main") fn main()->i64 {0}
"#,
    );
    let ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("io.file"),
        arguments: Vec::new(),
    };
    let mut expected = BTreeSet::new();
    for _ in 0..16 {
        expected.extend(saved_effects(&program, &ty).unwrap());
    }
    let mut actual = BTreeSet::new();
    for _ in 0..16 {
        extend_resolved_lifecycle_effects(&program, &ty, &mut actual).unwrap();
    }
    assert_eq!(actual, expected);
    assert_eq!(
        actual.iter().map(String::as_str).collect::<Vec<_>>(),
        ["filesystem.audit.record", "filesystem.handle.release"]
    );
    let missing = ResolvedType::Nominal {
        declaration: DeclarationId::new("io.absent"),
        arguments: Vec::new(),
    };
    let old = saved_effects(&program, &missing).unwrap_err();
    let new = extend_resolved_lifecycle_effects(&program, &missing, &mut actual).unwrap_err();
    assert_eq!((new.code, new.message), (old.code, old.message));
    assert_eq!(actual, expected);
    crate::hir::validate(&program).unwrap();
    let mut forged = program.clone();
    forged.functions[0].effects.clear();
    let error = crate::hir::validate(&forged).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "function `io.consume` omits lifecycle effect `filesystem.audit.record`"
    );
    let (_, old) = crate::test_allocation_observer::measure(|| {
        let mut effects = BTreeSet::new();
        for _ in 0..16 {
            effects.extend(saved_effects(&program, &ty).unwrap());
        }
        assert_eq!(effects, expected);
        drop(effects);
    });
    let (_, new) = crate::test_allocation_observer::measure(|| {
        let mut effects = BTreeSet::new();
        for _ in 0..16 {
            extend_resolved_lifecycle_effects(&program, &ty, &mut effects).unwrap();
        }
        assert_eq!(effects, expected);
        drop(effects);
    });
    assert!(new.allocated < old.allocated);
    assert_eq!((new.live, old.live), (0, 0));
    assert!(!new.invalid && !old.invalid);
}
