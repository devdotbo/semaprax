//! Independent old admission predicates remain the comparison oracle.
use super::*;
#[test]
fn paid_renewal_element_probes_match_old_families_and_refuse_before_unknown_lookup() {
    let source = crate::check(r#"
module renewal_probes;
@id("copy") record CopyRow { @id("copy.n") n:i64, }
@id("leaves") record Leaves { @id("leaves.n") n:i64, @id("leaves.s") text:string, @id("leaves.b") data:Bytes, }
@id("old") record OldRow { @id("old.a") a:Bytes, @id("old.n") n:i64, @id("old.b") b:Bytes, }
@id("main") fn main()->i64 {0}
"#, "renewal-probes.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let index = &program.declarations;
    let elements = [
        ResolvedType::I64,
        ResolvedType::Bytes,
        ResolvedType::String,
        ResolvedType::Nominal {
            declaration: super::super::DeclarationId::new("copy"),
            arguments: Vec::new(),
        },
        ResolvedType::Nominal {
            declaration: super::super::DeclarationId::new("leaves"),
            arguments: Vec::new(),
        },
        ResolvedType::Nominal {
            declaration: super::super::DeclarationId::new("old"),
            arguments: Vec::new(),
        },
        ResolvedType::Nominal {
            declaration: super::super::DeclarationId::new("absent-but-long-identifier"),
            arguments: Vec::new(),
        },
    ];
    for element in &elements {
        for op in crate::vec_ops::ALL {
            let mut work = |_| true;
            assert_eq!(
                operation_with_work(index, op, element, &mut work).unwrap(),
                vec_operation_admitted(index, op, element)
            );
        }
        assert_eq!(
            runtime_with_work(index, element, &mut |_| true).unwrap(),
            runtime_element(index, element)
        );
    }
    let query = super::super::DeclarationId::new("absent-but-long-identifier");
    // All immutable keys pay one census visit plus their actual comparison
    // byte bound. This literal receiver formula does not call lookup().
    let exact: usize = index
        .declarations()
        .map(|key| 1 + key.id.as_str().len().min(query.as_str().len()) + 1)
        .sum();
    for units in [exact, exact - 1] {
        let mut remaining = units;
        let mut failed = false;
        let mut work = |count| {
            if failed {
                return false;
            }
            match remaining.checked_sub(count) {
                Some(left) => {
                    remaining = left;
                    true
                }
                None => {
                    failed = true;
                    false
                }
            }
        };
        let result = super::super::renewal_probes::lookup(&index.declarations, &query, &mut work);
        if units == exact {
            assert!(result.unwrap().is_none());
        } else {
            assert!(result.is_err());
            assert!(!work(0));
        }
    }
    let forged = ResolvedType::Nominal {
        declaration: super::super::DeclarationId::new("leaves"),
        arguments: vec![ResolvedType::I64],
    };
    assert!(
        !operation_with_work(index, crate::vec_ops::VecOp::Push, &forged, &mut |_| true).unwrap()
    );
    assert_eq!(
        layout_with_work(index, &elements[4], &mut |_| false).map(|layout| layout.is_some()),
        Err(())
    );
}
