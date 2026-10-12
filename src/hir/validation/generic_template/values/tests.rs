//! Saved owned-map storage policy and exact current symbolic validation.
use super::*;
use crate::test_allocation_observer::measure;
const SOURCE: &str = r#"module test.generic_values;
@id("c.pair") record Pair<A,B> { @id("c.payload") payload:A, @id("c.marker") marker:B, }
@id("c.wrap") record Wrap<A> { @id("c.value") value:A, }
@id("c.rebuild") fn rebuild<T>(value:own Wrap<Pair<Bytes,T>>)->Wrap<Pair<Bytes,T>> {
    match own value {
        Wrap { value: Pair { payload, marker } } => Wrap<Pair<Bytes,T>> { value: Pair<Bytes,T> { payload:payload, marker:marker } },
    }
}
@id("c.local") fn local<T>(value:T)->T {
    let boxed=box_new<T>(value);
    let copied=box_get<T>(boxed);
    let values=vec_push<T>(vec_with_capacity<T>(1usize),copied);
    vec_get<T>(values,0usize)
}
@id("c.invoke") fn invoke(value:own Wrap<Pair<Bytes,bool>>)->Wrap<Pair<Bytes,bool>> {rebuild<bool>(value)}
@id("c.scalar") fn scalar(value:bool)->bool {local<bool>(value)}
@id("app.main") fn main()->i64 {0}
"#;
fn program() -> ResolvedProgram {
    let parsed = crate::check(SOURCE, "generic-values.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    let reparsed = crate::check(&canonical, "generic-values.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&reparsed));
    crate::hir::resolve(&reparsed).unwrap()
}
fn compare(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Result<(), Diagnostic> {
    let mut borrowed = HirValidator::new(program).unwrap();
    let mut owned = borrowed.clone();
    let execution = FunctionExecutionId::Monomorphic(template.id.clone());
    // Owned insert/snapshot/root are the saved original map operations. Both
    // routes execute the complete symbolic walk, not an env-only mock verdict.
    let old =
        owned.validate_template_expressions_with_values(template, &execution, Values::owned());
    let new = borrowed.validate_template_expressions(template, &execution);
    assert_eq!(new, old);
    assert_eq!(borrowed.expression_ids, owned.expression_ids);
    assert_eq!(borrowed.value_ids, owned.value_ids);
    new
}
#[test]
fn borrowed_template_types_preserve_owned_map_diagnostics_and_current_binding_substitutions() {
    let program = program();
    crate::hir::validate(&program).unwrap();
    assert_eq!(program.function_templates.len(), 2);
    for template in &program.function_templates {
        compare(&program, template).unwrap();
        // Two simultaneous defects retain the original header-first diagnostic.
        let mut wrong = template.clone();
        wrong.body.ownership = OwnershipMode::Borrow;
        wrong.body.ty = ResolvedType::String;
        assert_eq!(compare(&program, &wrong).unwrap_err().code, "SPX-H006");
        let mut foreign = template.clone();
        foreign.params[0].ty = ResolvedType::I64;
        assert_eq!(compare(&program, &foreign).unwrap_err().code, "SPX-H006");
    }
    let template = program
        .function_templates
        .iter()
        .find(|t| t.id.as_str() == "c.rebuild")
        .unwrap();
    let mut stale = template.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut stale.body.kind else {
        panic!("block");
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("record match");
    };
    let ResolvedMatchPattern::Record { fields, .. } = &mut arms[0].pattern else {
        panic!("record pattern");
    };
    fields[0].field = DeclarationId::new("foreign.field");
    assert_eq!(compare(&program, &stale).unwrap_err().code, "SPX-H006");
    for wrong_type in [true, false] {
        let mut wrong = template.clone();
        let ResolvedExprKind::Block { tail, .. } = &mut wrong.body.kind else {
            panic!("block");
        };
        let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
            panic!("match");
        };
        let ResolvedMatchPattern::Record { fields, .. } = &mut arms[0].pattern else {
            panic!("Wrap");
        };
        let ResolvedRecordMatchFieldPattern::Record { fields, .. } = &mut fields[0].pattern else {
            panic!("Pair");
        };
        let ResolvedRecordMatchFieldPattern::Binding(binding) = &mut fields[0].pattern else {
            panic!("payload");
        };
        if wrong_type {
            binding.ty = ResolvedType::String;
        } else {
            binding.ownership = OwnershipMode::Borrow;
        }
        // Fresh field substitution must reject before this binding's type can
        // enter either environment, regardless of its immutable source address.
        assert_eq!(compare(&program, &wrong).unwrap_err().code, "SPX-H006");
    }
}
#[test]
fn borrowed_template_snapshots_remove_real_type_payloads_through_complete_drop() {
    let program = program();
    let parameter = &program
        .function_templates
        .iter()
        .find(|t| t.id.as_str() == "c.rebuild")
        .unwrap()
        .params[0];
    // Wrap<Pair<Bytes,T>> owns exactly two singleton/type-argument Vecs. The
    // map, shared identity, and immutable input are excluded in neither route;
    // source/program construction precedes this narrow env lifetime observer.
    let ResolvedType::Nominal { arguments, .. } = &parameter.ty else {
        panic!("Wrap");
    };
    assert_eq!(arguments.len(), 1);
    let ResolvedType::Nominal {
        arguments: inner, ..
    } = &arguments[0]
    else {
        panic!("Pair");
    };
    assert_eq!(inner.len(), 2);
    let (_, old) = measure(|| {
        let mut values = Values::owned();
        values.insert(parameter.id.clone(), &parameter.ty);
        for _ in 0..64 {
            let snapshot = values.clone();
            assert_eq!(snapshot.get(&parameter.id), Some(&parameter.ty));
            assert_eq!(
                snapshot.root(&parameter.id).unwrap().as_ref(),
                &parameter.ty
            );
            drop(snapshot);
        }
        drop(values);
    });
    let (_, new) = measure(|| {
        let mut values = Values::new();
        values.insert(parameter.id.clone(), &parameter.ty);
        for _ in 0..64 {
            let snapshot = values.clone();
            let ty = snapshot.get(&parameter.id).unwrap();
            assert!(std::ptr::eq(ty, &parameter.ty));
            assert!(matches!(
                snapshot.root(&parameter.id).unwrap(),
                Cow::Borrowed(_)
            ));
            drop(snapshot);
        }
        drop(values);
    });
    // One initial insertion,64 snapshot clones,64 root reads; each old copy
    // allocates 1+2 ResolvedType slots. BTree nodes additionally remain priced
    // by actual observations; there is no fabricated zero-allocation map claim.
    let removed_payload = (1 + 64 + 64) * 3 * std::mem::size_of::<ResolvedType>();
    assert!(old.allocated >= new.allocated + removed_payload);
    assert!(old.allocations >= new.allocations + (1 + 64 + 64) * 2);
    for observation in [old, new] {
        assert!(!observation.invalid);
        assert_eq!(observation.live, 0);
        assert_eq!(observation.allocated, observation.freed);
    }
}

#[test]
fn generic_closure_scopes_keep_duplicate_priority_and_full_owned_oracle_policy() {
    let ty = ResolvedType::String;
    let id = ValueId::new("duplicate");
    for parent in [Values::new(), Values::owned()] {
        let mut empty = parent.empty_like();
        assert!(matches!(
            (&parent, &empty),
            (Values::Borrowed(_), Values::Borrowed(_)) | (Values::Owned(_), Values::Owned(_))
        ));
        assert!(!empty.insert(id.clone(), &ty));
        assert!(empty.insert(id.clone(), &ty));
        assert_eq!(empty.get(&id), Some(&ty));
    }
    // Same admitted symbolic closure shape as the retained language owner.
    let ast = crate::parse(
        r#"module test.generic_template_closure;
@id("gc.fill") fn fill<T>(values:own Vec<T>,replacement:T)->Vec<T>{
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
@id("gc.main") fn main()->i64{0}
"#,
        "borrowed-generic-closure.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    for template in &program.function_templates {
        compare(&program, template).unwrap();
        let mut hostile = template.clone();
        hostile.body.ty = ResolvedType::String;
        assert_eq!(compare(&program, &hostile).unwrap_err().code, "SPX-H006");
    }
}
