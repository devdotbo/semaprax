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
    let mut work = usize::MAX;
    let owner = ValidatedPartitionProgram::prepare(&source, &mut work).unwrap();
    assert!(std::ptr::eq(owner.source(), &source));
    assert!(super::super::validate(owner.program()).is_err());
    let function = &owner.program().functions[0];
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else { panic!("block") };
    let operands = crate::construction_fusion::partition::operation::typed(tail, &mut work).unwrap().unwrap();
    let ResolvedExprKind::Call { args, .. } = &tail.kind else { panic!("call") };
    assert!(std::ptr::eq(operands.owner, &args[0]));
    assert!(function.cleanup_plan.status_sources.iter().any(|status| status.id.lane == crate::cleanup_plan::StatusLane::OperationFailure));
    let mut probe = usize::MAX;
    owner.revalidate(&mut probe).unwrap();
    let used = usize::MAX - probe;
    let mut exact = used;
    owner.revalidate(&mut exact).unwrap();
    assert_eq!(exact, 0);
    let mut short = used - 1;
    assert!(owner.revalidate(&mut short).is_err());
    assert_eq!(short, 0);
}
#[test]
fn private_owner_refuses_foreign_carriers_and_actual_call_or_plan_mutations() {
    for source in [SOURCE.replace("fn split()", "fn split<T>()"),
        SOURCE.replace("bytes_partition(buffer,3usize,2usize)", "bytes_partition(bytes_zeroed(6usize),3usize,2usize)")]
    {
        let source = crate::parse(&source, "partition_checked.spx").unwrap();
        assert!(ValidatedPartitionProgram::prepare(&source, &mut usize::MAX).is_err());
    }
    let mut forbidden = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    forbidden.permits.push("foreign".to_owned());
    assert!(ValidatedPartitionProgram::prepare(&forbidden, &mut usize::MAX).is_err());
    let source = crate::parse(SOURCE, "partition_checked.spx").unwrap();
    for mutation in 0..4 {
        let mut owner = ValidatedPartitionProgram::prepare(&source, &mut usize::MAX).unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut owner.program.functions[0].body.kind else { panic!("block") };
        let ResolvedExprKind::Call { callee, type_arguments, args, .. } = &mut tail.kind else { panic!("call") };
        match mutation {
            0 => *callee = DeclarationId::new("foreign.partition"),
            1 => type_arguments.push(ResolvedType::Bytes),
            2 => args[0].ownership = OwnershipMode::Borrow,
            _ => owner.program.functions[0].cleanup_plan.status_sources.clear(),
        }
        let mut work = usize::MAX;
        assert!(owner.revalidate(&mut work).is_err(), "mutation {mutation}");
        assert_eq!(work, 0);
    }
}
