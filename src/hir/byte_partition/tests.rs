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
    assert!(crate::check(SOURCE, "partition_checked.spx").is_err());
    let source = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    assert!(ValidatedPartitionProgram::compile_source(&source, &mut 0).is_err());
    let mut work = usize::MAX;
    let owner = ValidatedPartitionProgram::compile_source(&source, &mut work).unwrap();
    assert!(std::ptr::eq(owner.source(), &source));
    owner.revalidate_compiler().unwrap();
    assert!(super::super::validate(owner.program()).is_err());
    let function = &owner.program().functions[0];
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else { panic!("block") };
    let operands = crate::construction_fusion::partition::operation::typed(tail, &mut work).unwrap().unwrap();
    let ResolvedExprKind::Call { args, .. } = &tail.kind else { panic!("call") };
    assert!(std::ptr::eq(operands.owner, &args[0]));
    assert!(function.cleanup_plan.status_sources.iter().any(|status| status.id.lane == crate::cleanup_plan::StatusLane::OperationFailure));
    let mut probe = usize::MAX;
    owner.receive_current_graph(&mut probe).unwrap();
    let used = usize::MAX - probe;
    let mut exact = used;
    owner.receive_current_graph(&mut exact).unwrap();
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(owner.receive_current_graph(&mut short).is_err());
    assert_eq!(short, 0);
}
#[test]
fn private_owner_refuses_foreign_carriers_and_actual_call_or_plan_mutations() {
    for source in [SOURCE.replace("fn split()", "fn split<T>()"),
        SOURCE.replace("bytes_partition(buffer,3usize,2usize)", "bytes_partition(bytes_zeroed(6usize),3usize,2usize)")]
    {
        let source = crate::parse(&source, "partition_checked.spx").unwrap();
        assert!(ValidatedPartitionProgram::compile_source(&source, &mut usize::MAX).is_err());
    }
    let alias = SOURCE.replace("let buffer=bytes_zeroed", "let original=bytes_zeroed")
        .replace("bytes_partition(buffer,3usize,2usize)", "let buffer=original; bytes_partition(buffer,3usize,2usize)");
    let alias = crate::parse(&alias, "partition_checked.spx").unwrap();
    ValidatedPartitionProgram::compile_source(&alias, &mut usize::MAX).unwrap();
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
    assert!(crate::source_verify::verify_partition(&child_helper).iter().all(|d| !d.severity.is_error()));
    let error = ValidatedPartitionProgram::compile_source(&child_helper, &mut usize::MAX).err().expect("child helper must refuse");
    assert!(error.message.contains("whole producer"), "{error:?}");
    let mut forbidden = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    forbidden.permits.push("foreign".to_owned());
    assert!(ValidatedPartitionProgram::compile_source(&forbidden, &mut usize::MAX).is_err());
    let source = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    for mutation in 0..4 {
        let mut owner = ValidatedPartitionProgram::compile_source(&source, &mut usize::MAX).unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut owner.program.functions[0].body.kind else { panic!("block") };
        let ResolvedExprKind::Call { callee, type_arguments, args, .. } = &mut tail.kind else { panic!("call") };
        match mutation {
            0 => *callee = DeclarationId::new("foreign.partition"),
            1 => type_arguments.push(ResolvedType::Bytes),
            2 => args[0].ownership = OwnershipMode::Borrow,
            _ => owner.program.functions[0].cleanup_plan.status_sources.clear(),
        }
        let mut work = usize::MAX;
        assert!(owner.revalidate_compiler().is_err() || owner.receive_current_graph(&mut work).is_err(), "mutation {mutation}");
        if mutation != 3 { assert!(owner.receive_current_graph(&mut work).is_err()); assert_eq!(work, 0); }
    }
}
