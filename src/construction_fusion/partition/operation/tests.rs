use super::{fault_code, prepare, source, typed, STATUS_DOMAIN};
use crate::construction_fusion::{
    domain::Refusal,
    partition::{Fault, Preparation},
};
use crate::{ast, hir};
fn call(program: &hir::ResolvedProgram) -> &hir::ResolvedExpr {
    let hir::ResolvedExprKind::Block { tail, .. } = &program.functions[0].body.kind else {
        panic!("body")
    };
    tail
}
#[test]
fn dynamic_operands_are_borrowed_and_every_shape_mutant_refuses() {
    let (source_program, program) = super::super::tests::pending();
    let call = call(&program);
    let mut work = usize::MAX;
    let operands = typed(call, &mut work).unwrap().unwrap();
    let hir::ResolvedExprKind::Call { args, .. } = &call.kind else {
        panic!("call")
    };
    assert!(std::ptr::eq(operands.owner, &args[0]));
    assert!(std::ptr::eq(operands.count, &args[1]));
    assert!(std::ptr::eq(operands.width, &args[2]));
    let cost = usize::MAX - work;
    let mut exact = cost;
    assert!(typed(call, &mut exact).unwrap().is_some());
    assert_eq!(exact, 0);
    let mut short = cost - 1;
    assert!(typed(call, &mut short).is_err());
    assert_eq!(short, 0);
    for mutation in 0..10 {
        let mut forged = call.clone();
        let hir::ResolvedExprKind::Call {
            type_arguments,
            args,
            ..
        } = &mut forged.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => type_arguments.push(hir::ResolvedType::Bytes),
            1 => {
                args.pop();
            }
            2 => args[0].ownership = hir::OwnershipMode::Borrow,
            3 => args[0].ty = hir::ResolvedType::SliceU8,
            4 => args[0].kind = hir::ResolvedExprKind::Usize(0),
            5 => args[1].ty = hir::ResolvedType::I64,
            6 => args[2].ownership = hir::OwnershipMode::Own,
            7 => forged.ownership = hir::OwnershipMode::Value,
            8 => forged.ty = hir::ResolvedType::Bytes,
            _ => {
                let hir::ResolvedType::Nominal { declaration, .. } = &mut forged.ty else {
                    unreachable!()
                };
                *declaration = hir::DeclarationId::new("foreign.Vec");
            }
        }
        let mut budget = usize::MAX;
        assert!(typed(&forged, &mut budget).is_err(), "mutation {mutation}");
        assert_eq!(budget, 0, "mutation {mutation}");
    }
    let ast::ExprKind::Block { tail, .. } = &source_program.functions[0].body.kind else {
        panic!("source")
    };
    assert!(source(tail, &mut usize::MAX).unwrap().is_some());
    let mut forged = tail.as_ref().clone();
    let ast::ExprKind::Call { args, .. } = &mut forged.kind else {
        panic!("source call")
    };
    args[0] = args[1].clone();
    assert!(source(&forged, &mut usize::MAX).is_err());
    // A shape fact cannot register the builtin or turn pending HIR into a lease.
    assert!(crate::check("module closed; @id(\"closed.main\") fn main()->i64{let b=bytes_zeroed(0usize);let v=bytes_partition(b,0usize,1usize);0}", "closed.spx").is_err());
}
#[test]
fn dynamic_geometry_keeps_semantic_priority_and_existing_limits() {
    assert_eq!(STATUS_DOMAIN, "semaprax.byte-partition.v1");
    for (extent, count, width, expected) in [
        (1, u64::MAX, 0, Fault::WidthZero),
        (1, u64::MAX, 2, Fault::ExtentOverflow),
        (1, 8193, 1, Fault::ExtentMismatch),
        (8193, 8193, 1, Fault::MetadataCapacity),
    ] {
        assert_eq!(
            prepare(extent, count, width),
            Err(Preparation::Fault(expected))
        );
    }
    assert_eq!(fault_code(Fault::WidthZero), 1);
    assert_eq!(fault_code(Fault::ExtentOverflow), 2);
    assert_eq!(fault_code(Fault::ExtentMismatch), 3);
    assert_eq!(fault_code(Fault::MetadataCapacity), 4);
    assert!(prepare(0, 0, u64::MAX).is_ok());
    assert!(prepare(131072, 8192, 16).is_ok());
    assert_eq!(
        prepare(131073, 1, 131073),
        Err(Preparation::Authority(Refusal::Range))
    );
    // Not a backing constructor: no event/payload account exists in preflight.
    assert_eq!(crate::byte_data_capacity::MAX_BYTES_COPY_SITES, 32);
    assert_eq!(
        crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES,
        2097152
    );
}
