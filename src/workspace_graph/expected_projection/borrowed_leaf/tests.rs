use super::{
    construct, eligible, private_layout, reserve_builder_structure,
    synthetic_program_with_registry, Input,
};
use crate::ast::program_source::ProgramSource;
use crate::ast::TypeDeclaration;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::workspace_graph::index_authored;
use std::fmt::Write;
const SOURCE: &str =
    "module borrowed_leaf; @id(\"read\") fn read(data:borrow Slice<u8>)->usize { byte_len(data) }";

#[test]
fn borrowed_leaf_preserves_owned_resolution_and_actual_authored_pointers() {
    for source in [SOURCE,
        "module borrowed_leaf; @id(\"main\") fn main()->i64 { let x=3; if x>0 { x+1 } else { 0 } }",
        "module borrowed_leaf; @id(\"read\") fn read(data:borrow Slice<u8>)->usize requires byte_len(data)>0usize { byte_len(data) }",
        "module borrowed_leaf; @id(\"row\") record Row { @id(\"row.value\") value:i64, } @id(\"main\") fn main()->i64 {0}",
    ] {
        let programs = [crate::parse(source, "leaf.spx").unwrap()];
        let authored = index_authored(&programs).unwrap();
        let registry = private_layout::registry::LayoutRegistry::new(&programs, &authored);
        let view = construct(&programs[0]).unwrap();
        assert!(std::ptr::eq(view.functions().get(0).unwrap(), &programs[0].functions[0]));
        assert!(std::ptr::eq(&view.functions().get(0).unwrap().body, &programs[0].functions[0].body));
        let other = construct(&programs[0]).unwrap();
        assert!(!view.subject().same(other.subject()));
        assert!(!view.subject().same(programs[0].subject()));
        let owned = synthetic_program_with_registry(&programs[0], &authored, &programs, Some(&registry)).unwrap();
        let expected = hir::resolve_with_function_reuse(&owned, None, None, None, None).unwrap().0;
        let actual = hir::resolve_source(&view, None).unwrap().0;
        assert_eq!(actual, expected);
        // Complete independent HIR/cleanup/status validation is retained.
        hir::validate(&actual).unwrap();
    }
}

#[test]
fn borrowed_leaf_drop_has_no_allocation_and_constructor_limits_are_exact_and_sticky() {
    let program = crate::parse(SOURCE, "leaf.spx").unwrap();
    let (view, overflowed, required) =
        crate::bounded_output::with_limit_usage(usize::MAX, || construct(&program));
    assert!(!overflowed);
    let view = view.unwrap();
    let (_, drop_metrics) = crate::test_allocation_observer::measure(|| drop(view));
    assert!(!drop_metrics.invalid);
    assert_eq!(drop_metrics.allocations, 0);
    assert_eq!(drop_metrics.reallocations, 0);
    assert!(required > 0);
    let ((result, allocation), overflowed, used) =
        crate::bounded_output::with_limit_usage(required, || {
            crate::test_allocation_observer::measure(|| {
                let view = construct(&program)?;
                drop(view);
                Ok::<_, Vec<Diagnostic>>(())
            })
        });
    result.unwrap();
    assert!(!overflowed);
    assert_eq!(used, required);
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert!(allocation.allocated <= required);
    let ((first, retry), overflowed, _) =
        crate::bounded_output::with_limit_usage(required - 1, || {
            let first = construct(&program);
            let retry = construct(&program);
            (first.is_err(), retry.is_err())
        });
    assert!(overflowed);
    assert!(first && retry);

    // Existing main removes generated-main overreservation. The independent
    // literal census has exactly 96 borrowed directory entries and no owned
    // expression/declaration subtree or sorting scratch.
    let mut source = String::from("module wide_leaf;");
    for index in 0..96 {
        let name = index * 37 % 96;
        write!(
            source,
            " @id(\"row.{index}\") record Row{name} {{ @id(\"row.{index}.value\") value:i64, }}"
        )
        .unwrap();
    }
    source.push_str(" @id(\"main\") fn main()->i64 {0}");
    let wide = crate::parse(&source, "wide-leaf.spx").unwrap();
    let programs = std::slice::from_ref(&wide);
    let authored = index_authored(programs).unwrap();
    let registry = private_layout::registry::LayoutRegistry::new(programs, &authored);
    let directory = 96 * std::mem::size_of::<(&str, &TypeDeclaration)>();
    assert_eq!(
        super::ast_peak(&wide).unwrap(),
        directory + std::mem::size_of::<Input<'_>>()
    );
    let ((result, allocation), overflowed, used) =
        crate::bounded_output::with_limit_usage(directory, || {
            crate::test_allocation_observer::measure(|| {
                let view = Input::construct(&wide, &authored, programs, &registry, true)?;
                assert_eq!(view.source().types().len(), 96);
                for (actual, authored) in view.source().types().iter().zip(&wide.types) {
                    assert!(std::ptr::eq(actual, authored));
                }
                drop(view);
                Ok::<_, Vec<Diagnostic>>(())
            })
        });
    result.unwrap();
    assert!(!overflowed);
    assert_eq!(used, directory);
    assert!(!allocation.invalid);
    assert_eq!(allocation.allocated, directory);
    assert_eq!(allocation.reallocations, 0);
    assert_eq!(allocation.live, 0);
    let ((first, retry), overflowed, _) =
        crate::bounded_output::with_limit_usage(directory - 1, || {
            (
                Input::construct(&wide, &authored, programs, &registry, true).is_err(),
                Input::construct(&wide, &authored, programs, &registry, true).is_err(),
            )
        });
    assert!(overflowed);
    assert!(first && retry);
}

#[test]
fn borrowed_leaf_full_pipeline_keeps_owned_fallback_and_original_diagnostics() {
    let originals = [crate::parse(SOURCE, "leaf.spx").unwrap()];
    let original_authored = index_authored(&originals).unwrap();
    let mut owned =
        synthetic_program_with_registry(&originals[0], &original_authored, &originals, None)
            .unwrap();
    let mut program = originals[0].clone();
    let duplicate = program.functions[0].clone();
    owned.functions.insert(1, duplicate.clone());
    program.functions.push(duplicate);
    let view = construct(&program).unwrap();
    assert_eq!(
        hir::resolve_source(&view, None).err(),
        hir::resolve_with_function_reuse(&owned, None, None, None, None).err()
    );

    let duplicate_types = [crate::parse(
        "module duplicate_types; @id(\"first\") record Row { @id(\"first.value\") value:i64, } @id(\"second\") record Row { @id(\"second.value\") value:i64, } @id(\"duplicate.main\") fn main()->i64 { 0 }",
        "duplicate_types.spx",
    ).unwrap()];
    let type_authored = index_authored(&duplicate_types).unwrap();
    let old_error = synthetic_program_with_registry(
        &duplicate_types[0],
        &type_authored,
        &duplicate_types,
        None,
    )
    .err()
    .unwrap();
    let new_error = construct(&duplicate_types[0]).err().unwrap();
    assert_eq!(new_error, old_error);
    assert_eq!(new_error[0].code, "SPX-G173");

    let programs = [crate::parse(SOURCE, "leaf.spx").unwrap()];
    let authored = index_authored(&programs).unwrap();
    let registry = private_layout::registry::LayoutRegistry::new(&programs, &authored);
    let prepared_owned =
        synthetic_program_with_registry(&programs[0], &authored, &programs, Some(&registry))
            .unwrap();
    drop(hir::resolve(&prepared_owned).unwrap());
    drop(prepared_owned);
    let ((result, allocation), overflowed, used) =
        crate::bounded_output::with_limit_usage(super::super::super::MAX_BUILDER_BYTES, || {
            crate::test_allocation_observer::measure(|| {
                let costs = super::super::synthetic_builder_bytes_scoped_with_registry(
                    &programs[0],
                    &authored,
                    &programs,
                    None,
                    4,
                    true,
                    Some(&registry),
                )?;
                reserve_builder_structure(costs.raw_clone_and_hir)?;
                let view = construct(&programs[0])?;
                let resolved = hir::resolve_source(&view, None)?.0;
                hir::validate(&resolved).map_err(|error| vec![error])?;
                drop(resolved);
                drop(view);
                Ok::<_, Vec<Diagnostic>>(())
            })
        });
    result.unwrap();
    assert!(!overflowed);
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert!(allocation.highwater <= used);
    // Cache construction retains the independent ordinary owned key.
    let cached = Input::construct(&programs[0], &authored, &programs, &registry, false).unwrap();
    assert!(cached.owned().is_some());
    let delta = super::input_carrier_delta().unwrap();
    // The enum may reuse a niche in Program; zero extra inline bytes is valid.
    assert!(std::mem::size_of::<Input<'_>>() >= std::mem::size_of::<crate::ast::Program>());
    assert_eq!(
        delta,
        std::mem::size_of::<Input<'_>>() - std::mem::size_of::<crate::ast::Program>()
    );
    let uncached = Input::construct(&programs[0], &authored, &programs, &registry, true).unwrap();
    assert!(uncached.owned().is_none());
    let imported = crate::parse("module imported; use function @id(\"read\") from borrowed_leaf as read; @id(\"main\") fn main()->i64 {0}", "imported.spx").unwrap();
    assert!(!eligible(&imported));
    assert!(construct(&imported).is_err());
}
