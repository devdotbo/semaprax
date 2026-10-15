use super::*;
#[test]
fn inline_type_frames_keep_complete_vec_order_and_unbounded_spill_drop() {
    for count in [0, 1, 8, 9, 16, 256] {
        let mut old = Vec::new();
        let mut current = Stack::<_, 8>::new();
        for n in 0..count {
            old.push(n);
            current.push(n);
        }
        for n in 0..count {
            assert_eq!(current.pop(), old.pop());
            if n % 3 == 0 {
                old.push(1000 + n);
                current.push(1000 + n);
                assert_eq!(current.pop(), old.pop());
            }
        }
        assert_eq!(current.pop(), old.pop());
    }
    for count in [1, 8, 9, 256] {
        let (_, actual) = crate::test_allocation_observer::measure(|| {
            let mut stack = Stack::<_, 8>::new();
            for n in 0..count {
                stack.push(n);
            }
            while let Some(n) = stack.pop() {
                std::hint::black_box(n);
            }
        });
        assert!(!actual.invalid);
        assert_eq!(actual.live, 0);
        assert_eq!(actual.allocated, actual.freed);
        if count <= 8 {
            assert_eq!((actual.allocated, actual.allocations), (0, 0));
        } else {
            assert!(actual.allocated > 0);
        }
    }
}
#[test]
fn inline_final_type_receiver_matches_complete_original_and_eager_hostile_verdicts() {
    use crate::hir::validation::HirValidator;
    use crate::hir::{DeclarationId, ResolvedType};
    let source=crate::parse(r#"module inline.types; @id("generic") record Generic<T> { @id("field") field:T, } @id("app.main") fn main()->i64{0}"#,std::path::Path::new("types.spx")).unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let validator = HirValidator::new(&program).unwrap();
    let mut types = vec![
        ResolvedType::I64,
        ResolvedType::Nominal {
            declaration: DeclarationId::new("generic"),
            arguments: vec![ResolvedType::I64],
        },
        ResolvedType::Nominal {
            declaration: DeclarationId::new("generic"),
            arguments: vec![],
        },
        ResolvedType::Nominal {
            declaration: DeclarationId::new("forged"),
            arguments: vec![ResolvedType::String],
        },
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("foreign"),
            index: 0,
        },
    ];
    for arity in [0, 1, 8, 9, 64] {
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::I64; arity],
            result: Box::new(ResolvedType::Bool),
        });
    }
    for ty in &types {
        let actual = validator.validate_type(ty);
        assert_eq!(actual, validator.validate_type_original_receiver(ty));
        assert_eq!(actual, validator.validate_type_eager_oracle(ty));
    }
}
