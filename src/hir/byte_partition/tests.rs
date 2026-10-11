use super::*;
const SOURCE: &str = r#"module partition_checked;
@id("partition_checked.split") fn split()->Vec<Bytes>{
 let buffer=bytes_zeroed(6usize);
 bytes_partition(buffer,3usize,2usize)
}
@id("partition_checked.main") fn main()->i64{
 let values=split();
 if vec_len<Bytes>(values)==3usize {7}else{0}
}
"#;
#[test]
fn original_source_resolves_only_through_the_private_current_owner() {
    let ordinary = crate::check(SOURCE, "partition_checked.spx").unwrap_err();
    assert!(ordinary.iter().any(|d| d.code == "SPX-T263"));
    let source = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&source)
        .iter()
        .all(|d| !d.severity.is_error()));
    // The result remains affine, and borrowing still needs a named carrier.
    let moved = SOURCE.replace(
        "if vec_len<Bytes>(values)==3usize {7}else{0}",
        "let transferred=vec_into_iter<Bytes>(values);if vec_len<Bytes>(values)==3usize {7}else{0}",
    );
    let moved = crate::parse(&moved, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&moved)
        .iter()
        .any(|d| d.severity.is_error()));
    let unnamed = SOURCE.replace(
        "if vec_len<Bytes>(values)==3usize {7}else{0}",
        "if vec_len<Bytes>(split())==3usize {7}else{0}",
    );
    let unnamed = crate::parse(&unnamed, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&unnamed)
        .iter()
        .any(|d| d.code == "SPX-O118"));
    assert!(ValidatedPartitionProgram::compile_source(&source, &mut 0).is_err());
    let mut work = usize::MAX;
    let owner = ValidatedPartitionProgram::compile_source(&source, &mut work).unwrap();
    assert!(std::ptr::eq(owner.source(), &source));
    owner.revalidate_compiler().unwrap();
    assert!(super::super::validate(owner.program()).is_err());
    let function = &owner.program().functions[0];
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        panic!("block")
    };
    let operands = crate::construction_fusion::partition::operation::typed(tail, &mut work)
        .unwrap()
        .unwrap();
    let ResolvedExprKind::Call { args, .. } = &tail.kind else {
        panic!("call")
    };
    assert!(std::ptr::eq(operands.owner, &args[0]));
    assert!(function
        .cleanup_plan
        .status_sources
        .iter()
        .any(|status| status.id.lane == crate::cleanup_plan::StatusLane::OperationFailure));
    let mut probe = usize::MAX;
    owner.receive_current_graph(&mut probe).unwrap();
    let used = usize::MAX - probe;
    let mut exact = used;
    owner.receive_current_graph(&mut exact).unwrap();
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(owner.receive_current_graph(&mut short).is_err());
    assert_eq!(short, 0);

    let inherited = SOURCE.replace(
        "@id(\"partition_checked.main\")",
        r#"
@id("partition_checked.inherited") fn inherited(input:borrow Bytes)->usize {
 let alias=bytes_as_slice(input);byte_len(alias)
}
@id("partition_checked.main")"#,
    );
    let inherited = crate::parse(&inherited, "partition_checked.spx").unwrap();
    let inherited =
        ValidatedPartitionProgram::compile_source(&inherited, &mut { usize::MAX }).unwrap();
    let helper = inherited
        .program()
        .functions
        .iter()
        .find(|f| f.name == "inherited")
        .unwrap();
    assert!(
        helper.loan_plan.loans.is_empty(),
        "Borrow parameter descriptor cannot mint an Own-root loan"
    );
    assert_eq!(
        inherited
            .simultaneous_view_bound(helper, &mut { usize::MAX })
            .unwrap(),
        0
    );

    // Four live caller frames over four whole backings. Each caller retains
    // its 32 independent SliceView sites until after the nested call returns.
    // Each later byte_len read also has one canonical BorrowedCall site.
    for (extra_read, expected) in [(false, Some(256)), (true, None)] {
        let text = view_chain(extra_read);
        let ast = crate::parse(&text, "partition_view_bound.spx").unwrap();
        let owner = ValidatedPartitionProgram::compile_source(&ast, &mut { usize::MAX }).unwrap();
        let entry = &owner.program().functions[0];
        let counts: Vec<_> = owner
            .program()
            .functions
            .iter()
            .map(|f| f.loan_plan.loans.len())
            .collect();
        assert_eq!(counts, vec![64, 64, 64, 64 + usize::from(extra_read)]);
        let mut work = usize::MAX;
        match expected {
            Some(bound) => {
                assert_eq!(
                    owner.simultaneous_view_bound(entry, &mut work).unwrap(),
                    bound
                );
                let spent = usize::MAX - work;
                let mut exact = spent;
                assert_eq!(
                    owner.simultaneous_view_bound(entry, &mut exact).unwrap(),
                    bound
                );
                assert_eq!(exact, 0);
                let mut short = spent - 1;
                assert!(owner.simultaneous_view_bound(entry, &mut short).is_err());
                assert_eq!(short, 0);
                let clone = entry.clone();
                let mut clone_work = usize::MAX;
                assert!(owner
                    .simultaneous_view_bound(&clone, &mut clone_work)
                    .is_err());
                assert_eq!(clone_work, 0);
            }
            None => {
                let diagnostic = owner.simultaneous_view_bound(entry, &mut work).unwrap_err();
                assert!(diagnostic.message.contains("exceeds 256"), "{diagnostic:?}");
                assert_eq!(work, 0);
            }
        }
    }
}
#[test]
fn private_owner_refuses_foreign_carriers_and_actual_call_or_plan_mutations() {
    for source in [
        SOURCE.replace("fn split()", "fn split<T>()"),
        SOURCE.replace(
            "bytes_partition(buffer,3usize,2usize)",
            "bytes_partition(bytes_zeroed(6usize),3usize,2usize)",
        ),
    ] {
        let source = crate::parse(&source, "partition_checked.spx").unwrap();
        assert!(ValidatedPartitionProgram::compile_source(&source, &mut { usize::MAX }).is_err());
    }
    let alias = SOURCE
        .replace("let buffer=bytes_zeroed", "let original=bytes_zeroed")
        .replace(
            "bytes_partition(buffer,3usize,2usize)",
            "let buffer=original; bytes_partition(buffer,3usize,2usize)",
        );
    let alias = crate::parse(&alias, "partition_checked.spx").unwrap();
    ValidatedPartitionProgram::compile_source(&alias, &mut { usize::MAX }).unwrap();
    let child_helper = r#"module partition_checked;
@id("partition_checked.again") fn again(buffer:own Bytes)->Vec<Bytes>{bytes_partition(buffer,1usize,2usize)}
@id("partition_checked.main") fn main()->i64{
 let buffer=bytes_zeroed(4usize);
 let children=bytes_partition(buffer,2usize,2usize);
 match own iter_next<Bytes>(vec_into_iter<Bytes>(children)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>{let nested=again(item);if vec_len<Bytes>(nested)==1usize {7}else{0}},
 }
}
"#;
    let child_helper = crate::parse(child_helper, "partition_checked.spx").unwrap();
    assert!(crate::source_verify::verify_partition(&child_helper)
        .iter()
        .all(|d| !d.severity.is_error()));
    let error = ValidatedPartitionProgram::compile_source(&child_helper, &mut { usize::MAX })
        .err()
        .expect("child helper must refuse");
    assert!(error.message.contains("whole producer"), "{error:?}");
    let mut forbidden = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    forbidden.permits.push("foreign".to_owned());
    assert!(ValidatedPartitionProgram::compile_source(&forbidden, &mut { usize::MAX }).is_err());
    let source = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    for mutation in 0..4 {
        let mut owner =
            ValidatedPartitionProgram::compile_source(&source, &mut { usize::MAX }).unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut owner.program.functions[0].body.kind else {
            panic!("block")
        };
        let ResolvedExprKind::Call {
            callee,
            type_arguments,
            args,
            ..
        } = &mut tail.kind
        else {
            panic!("call")
        };
        match mutation {
            0 => *callee = DeclarationId::new("foreign.partition"),
            1 => type_arguments.push(ResolvedType::Bytes),
            2 => args[0].ownership = OwnershipMode::Borrow,
            _ => owner.program.functions[0]
                .cleanup_plan
                .status_sources
                .clear(),
        }
        let mut work = usize::MAX;
        assert!(
            owner.revalidate_compiler().is_err() || owner.receive_current_graph(&mut work).is_err(),
            "mutation {mutation}"
        );
        if mutation != 3 {
            assert!(owner.receive_current_graph(&mut work).is_err());
            assert_eq!(work, 0);
        }
        let mut bound_work = usize::MAX;
        assert!(owner
            .simultaneous_view_bound(&owner.program.functions[1], &mut bound_work)
            .is_err());
        assert_eq!(
            bound_work, 0,
            "failed compiler renewal must poison view receipt"
        );
    }
}

fn view_chain(extra_read: bool) -> String {
    use std::fmt::Write;
    let mut source = String::from("module partition_view_bound;\n");
    for frame in 0..4 {
        let name = if frame == 0 {
            "main".to_owned()
        } else {
            format!("frame{frame}")
        };
        writeln!(
            source,
            "@id(\"partition_view_bound.{name}\") fn {name}()->i64 {{"
        )
        .unwrap();
        source.push_str("let buffer=bytes_zeroed(1usize);\n");
        let views = if frame == 3 { 31 } else { 32 };
        for site in 0..views {
            writeln!(source, "let view{site}=bytes_as_slice(buffer);").unwrap();
        }
        if frame < 3 {
            writeln!(source, "let value=frame{}();", frame + 1).unwrap();
        }
        for site in 0..views {
            writeln!(source, "let observed{site}=byte_len(view{site});").unwrap();
        }
        if frame == 3 {
            source.push_str("let final_observed=byte_len(view0);\n");
            if extra_read {
                source.push_str("let extra_observed=byte_len(view0);\n");
            }
            source.push_str("let children=bytes_partition(buffer,1usize,1usize);if vec_len<Bytes>(children)==1usize {7}else{0}\n");
        } else {
            source.push_str("value\n");
        }
        source.push_str("}\n");
    }
    source
}
