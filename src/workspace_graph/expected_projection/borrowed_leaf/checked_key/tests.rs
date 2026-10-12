use super::{append_copies, materialize, reserve_builder_structure};
use super::super::{construct, private_layout, synthetic_program_with_registry, Input, Span};
use crate::hir;
use crate::ast::program_source::ProgramSource;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;

#[test]
fn cold_checked_key_matches_complete_owned_producer_and_moves_generated_main() {
    for source in [
        "module cold_leaf; @id(\"read\") fn read()->i64 {1}",
        "module cold_leaf; @id(\"main\") fn main()->i64 {let x=3; if x>0 {x+1} else {0}}",
        "module cold_leaf; @id(\"row\") record Row { @id(\"row.x\") x:i64, } @id(\"read\") fn read(value:i64)->i64 requires value>0 {value}",
    ] {
        let program = crate::parse(source, "cold-leaf.spx").unwrap();
        let programs = std::slice::from_ref(&program);
        let authored = index_authored(programs).unwrap();
        let registry = private_layout::registry::LayoutRegistry::new(programs, &authored);
        let old = synthetic_program_with_registry(&program, &authored, programs, Some(&registry)).unwrap();
        let view = construct(&program).unwrap();
        for (ordinal, function) in program.functions.iter().enumerate() {
            assert!(std::ptr::eq(view.functions().get(ordinal).unwrap(), function));
            assert!(std::ptr::eq(&view.functions().get(ordinal).unwrap().body, &function.body));
        }
        let generated = view.functions().get(program.functions.len()).map(|function| function.stable_id.as_ptr());
        let resolved = hir::resolve_source(&view, None).unwrap().0;
        assert_eq!(resolved, hir::resolve_with_function_reuse(&old, None, None, None, None).unwrap().0);
        let key = materialize(view).unwrap();
        assert_eq!(key, old);
        if let Some(pointer) = generated {
            assert_eq!(key.functions.last().unwrap().stable_id.as_ptr(), pointer);
        }
        for (owned, original) in key.functions.iter().zip(&program.functions) {
            assert!(!std::ptr::eq(owned, original));
            assert_eq!(owned, original);
        }
        // The old ephemeral-view rejection remains an independent negative.
        assert!(Input::Borrowed(construct(&program).unwrap()).into_owned().is_err());
    }
}

#[test]
fn checked_key_omits_only_discarded_sidecar_clone_payloads_and_preserves_sticky_refusal() {
    let mut program = crate::parse(
        "module cold_leaf; @id(\"main\") fn main()->i64 {0}",
        "cold-leaf.spx",
    )
    .unwrap();
    // This is a projection/storage witness, not source implementation authority.
    // Project validates real sidecars before either producer clears them.
    program
        .implementations
        .push(crate::ast::ProtocolImplementation {
            stable_id: "implementation".to_owned(),
            explicit_id: true,
            protocol_id: "protocol".to_owned(),
            receiver_id: "receiver".to_owned(),
            members: vec![crate::ast::ProtocolImplementationMember {
                method_id: "method".to_owned(),
                function_id: "function".to_owned(),
                span: Span::default(),
            }],
            span: Span::default(),
        });
    let view = construct(&program).unwrap();
    let (_, before) = measure(|| {
        let mut old = program.clone();
        old.module_uses.clear();
        old.implementations.clear();
        drop(old);
    });
    let (_, after) = measure(|| drop(materialize(view).unwrap()));
    let removed = std::mem::size_of::<crate::ast::ProtocolImplementation>()
        + std::mem::size_of::<crate::ast::ProtocolImplementationMember>()
        + "implementation".len()
        + "protocol".len()
        + "receiver".len()
        + "method".len()
        + "function".len();
    assert_eq!(before.allocations, after.allocations + 7);
    assert_eq!(before.reallocations, after.reallocations);
    assert_eq!(before.allocated, after.allocated + removed);
    for observed in [before, after] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    let view = construct(&program).unwrap();
    let ((failed, retry), overflow, charged, first) =
        crate::bounded_output::with_limit_usage_refusal(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            let failed = materialize(view).is_err();
            let retry = crate::bounded_output::reserve_active_required(0);
            (failed, retry)
        });
    assert!(failed && overflow && !retry);
    assert_eq!(charged, 0);
    assert!(first.is_some());
}


#[test]
fn imported_cache_directory_is_paid_while_generated_backing_is_live() {
    let authored = [11u64, 12];
    let bytes = 19 * std::mem::size_of::<u64>();
    for limit in [bytes, bytes - 1] {
        // Generated backing belongs to the already-paid overlay. Observe only
        // its real replacement; successful receiving Drop also frees it.
        let generated = vec![13u64; 17];
        let ((success, observed), refused, charged, first) =
            crate::bounded_output::with_limit_usage_refusal(limit, || measure(|| {
                let result = append_copies(&authored, generated);
                let success = result.is_ok();
                if let Ok(values) = &result {
                    assert_eq!(&values[..2], &authored);
                    assert_eq!(&values[2..], &[13u64; 17]);
                } else {
                    assert!(reserve_builder_structure(0).is_err());
                }
                drop(result);
                success
            }));
        // The allocation observer records frees only for its own allocations;
        // generated backing was acquired outside this receiver's observation.
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
        if limit == bytes {
            assert!(success && !refused && charged == bytes);
            assert_eq!(observed.allocations, 1);
            assert_eq!(observed.reallocations, 0);
            assert_eq!(observed.allocated, bytes);
            assert_eq!(observed.highwater, bytes);
        } else {
            assert!(!success && refused && first.is_some());
        }
    }
}
