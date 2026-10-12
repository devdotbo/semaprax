//! Identity leaves need no heap worklist; compound types keep the old walk.
use super::*;
use crate::test_allocation_observer::measure;
mod old;

#[test]
fn primitive_identity_audit_removes_only_the_old_worklist_and_keeps_dynamic_first_errors() {
    let leaves = [
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
        ResolvedType::Never,
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::ArrayU8(0),
        ResolvedType::ArrayU8(65_536),
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::String,
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::StringMap,
    ];
    for ty in &leaves {
        assert_eq!(audit_resolved_type(ty), old::audit_resolved_type(ty));
    }
    // Both observers enclose the complete query lifecycle including worklist
    // Drop. Fixture/type construction is outside these query scopes.
    let ((), current) = measure(|| {
        for _ in 0..1000 {
            for ty in &leaves {
                audit_resolved_type(ty).unwrap();
            }
        }
    });
    let ((), saved) = measure(|| {
        for _ in 0..1000 {
            for ty in &leaves {
                old::audit_resolved_type(ty).unwrap();
            }
        }
    });
    assert!(!current.invalid && !saved.invalid);
    assert_eq!(current.allocations, 0);
    assert_eq!(current.allocated, 0);
    assert_eq!(current.live, 0);
    // Independent original request: vec![root] owns one type reference.
    assert_eq!(saved.allocations, 1000 * leaves.len());
    assert_eq!(
        saved.allocated,
        1000 * leaves.len() * std::mem::size_of::<&ResolvedType>()
    );
    assert_eq!(saved.allocated, saved.freed);
    assert_eq!(saved.live, 0);

    let nominal = |id: &str, arguments| ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    };
    let dynamic = [
        nominal("unknown.current", vec![ResolvedType::I64]),
        nominal("outer", vec![nominal("bad\0nested", Vec::new())]),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("bad\0owner"),
            index: 0,
        },
        ResolvedType::Function {
            parameters: vec![nominal("bad\0parameter", Vec::new())],
            result: Box::new(ResolvedType::TypeParameter {
                owner: DeclarationId::new("bad\0result"),
                index: 0,
            }),
        },
    ];
    for ty in &dynamic {
        assert_eq!(audit_resolved_type(ty), old::audit_resolved_type(ty));
    }
    let error = audit_resolved_type(&dynamic[3]).unwrap_err();
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "resolved type-parameter owner identity contains NUL"
    );
    // Identity auditing does not grant type/index/entrypoint admission.
    // The ordinary full validator still rejects an unknown nominal result.
    let mut program = resolved(CALLS, "type-identity-audit.spx");
    let main = program
        .functions
        .iter_mut()
        .find(|f| f.name == "main")
        .unwrap();
    main.return_type = nominal("unknown.current", Vec::new());
    assert!(crate::hir::validate(&program).is_err());
}

#[test]
fn nominal_and_parameter_identity_leaves_keep_exact_nul_refusals_without_query_backing() {
    let nominal = |id: &str| ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments: Vec::new(),
    };
    let parameter = |id: &str| ResolvedType::TypeParameter {
        owner: DeclarationId::new(id),
        index: 3,
    };
    let accepted = [
        nominal("unknown.current"),
        nominal("葉"),
        parameter("owner"),
        parameter(""),
    ];
    for ty in &accepted {
        assert_eq!(audit_resolved_type(ty), old::audit_resolved_type(ty));
    }
    let ((), current) = measure(|| {
        for _ in 0..1000 {
            for ty in &accepted {
                audit_resolved_type(ty).unwrap();
            }
        }
    });
    let ((), saved) = measure(|| {
        for _ in 0..1000 {
            for ty in &accepted {
                old::audit_resolved_type(ty).unwrap();
            }
        }
    });
    assert!(!current.invalid && !saved.invalid);
    assert_eq!(
        (current.allocations, current.allocated, current.live),
        (0, 0, 0)
    );
    assert_eq!(saved.allocations, 1000 * accepted.len());
    assert_eq!(
        saved.allocated,
        1000 * accepted.len() * std::mem::size_of::<&ResolvedType>()
    );
    assert_eq!(saved.allocated, saved.freed);
    assert_eq!(saved.live, 0);
    for (ty, expected) in [
        (
            nominal("bad\0nominal"),
            "resolved nominal type identity contains NUL",
        ),
        (
            parameter("bad\0parameter"),
            "resolved type-parameter owner identity contains NUL",
        ),
    ] {
        let actual = audit_resolved_type(&ty);
        assert_eq!(actual, old::audit_resolved_type(&ty));
        let error = actual.unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert_eq!(error.message, expected);
    }
}
