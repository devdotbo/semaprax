//! Full overlay acceptance owners; the saved complete producer remains oracle.
use super::super::{construct, eligible, reserve_builder_structure, LayoutRegistry};
use super::{old, parity, sources};
use crate::ast::program_source::ProgramSource;
use crate::bounded_output::with_limit_usage_refusal;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::index_authored;

const VEC_PROVIDER: &str = include_str!("../../../../../std/collections/src/collections.spx");
const BOX_PROVIDER: &str = include_str!("../../../../../std/mem/src/mem.spx");
const VEC_CALLER: &str = r#"module overlay.vec;
use function @id("std.collections.vec.with-capacity") from std.collections as allocate;
use function @id("std.collections.vec.push") from std.collections as append;
use function @id("std.collections.vec.get") from std.collections as read;
@id("overlay.vec.main") fn main()->i64 { let values=allocate<i64>(2usize); let values=append<i64>(values,42); read<i64>(values,0usize) }"#;
const BOX_CALLER: &str = r#"module overlay.box;
use function @id("std.mem.box.new") from std.mem as allocate;
use function @id("std.mem.box.into-inner") from std.mem as consume;
@id("overlay.box.main") fn main()->i64 { consume<i64>(allocate<i64>(42)) }"#;
const CLASS_PROVIDER: &str = r#"module overlay.class_provider;
@id("overlay.counter") class Counter { @id("overlay.counter.value") value:i64,
@id("overlay.counter.bumped") fn bumped(self:Counter, amount:i64)->Counter { Counter { value:self.value+amount } } }"#;
const CLASS_CALLER: &str = r#"module overlay.class_app;
use type @id("overlay.counter") from overlay.class_provider as Counter;
@id("overlay.class.main") fn main()->i64 { let counter=Counter {value:41}; let updated=counter.bumped(1); updated.value }"#;
const INVARIANT_PROVIDER: &str = r#"module overlay.invariant_provider;
@id("overlay.row") record Row { @id("overlay.row.x") x:i64, } requires x>=0"#;
const INVARIANT_CALLER: &str = r#"module overlay.invariant_app;
use type @id("overlay.row") from overlay.invariant_provider as Imported;
@id("overlay.invariant.main") fn main()->i64 { let row=Imported{x:41}; let updated=row with {x:42}; updated.x }"#;

#[test]
fn retained_vec_box_class_and_invariant_sources_match_full_owning_projection() {
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_CALLER, INVARIANT_PROVIDER),
    ] {
        parity(caller, provider);
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        let owned = old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap();
        let view = construct(&programs[0], &authored, &programs, &registry).unwrap();
        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
        let mut old_requests = Vec::new();
        let old = hir::identity_resolution::resolve(
            &owned,
            layout.presentation(&programs[0], &owned).unwrap(),
            |bytes| {
                old_requests.push(bytes);
                Ok::<(), ()>(())
            },
        )
        .unwrap();
        let mut new_requests = Vec::new();
        let new = hir::identity_resolution::resolve_source(
            &view,
            layout.presentation(&programs[0], &view).unwrap(),
            |bytes| {
                new_requests.push(bytes);
                Ok::<(), ()>(())
            },
        )
        .unwrap();
        assert_eq!(old.0, new.0);
        assert_eq!(old.1.reused(), new.1.reused());
        assert_eq!(old.1.costs(), new.1.costs());
        assert_eq!(old_requests, new_requests);
        // No transformed/generated declaration changes authored identity.
        for (ordinal, function) in programs[0].functions.iter().enumerate() {
            assert!(std::ptr::eq(
                view.functions().get(ordinal).unwrap(),
                function
            ));
        }
    }
}

#[test]
fn invariant_and_wrapper_invalid_cases_keep_the_entire_old_error_order() {
    let cases = [
        (
            VEC_CALLER.replace("allocate<i64>(2usize)", "allocate<string>(2usize)"),
            VEC_PROVIDER.to_owned(),
        ),
        (
            BOX_CALLER.replace(
                "consume<i64>(allocate<i64>(42))",
                "consume<i64>(allocate<i64>(true))",
            ),
            BOX_PROVIDER.to_owned(),
        ),
        (
            CLASS_CALLER.replace("counter.bumped(1)", "counter.missing(1)"),
            CLASS_PROVIDER.to_owned(),
        ),
        (
            INVARIANT_CALLER.to_owned(),
            INVARIANT_PROVIDER.replace("requires x>=0", "requires missing>=0"),
        ),
        (
            INVARIANT_CALLER.to_owned(),
            INVARIANT_PROVIDER.replace("requires x>=0", "requires x"),
        ),
        (
            INVARIANT_CALLER.to_owned(),
            INVARIANT_PROVIDER
                .replace("x:i64", "x:Bytes")
                .replace("requires x>=0", "requires true"),
        ),
    ];
    for (caller, provider) in cases {
        let programs = sources(&caller, &provider);
        let authored = index_authored(&programs).unwrap();
        let registry = LayoutRegistry::new(&programs, &authored);
        assert!(eligible(&programs[0], &authored, &programs));
        let before = old::construct(&programs[0], &authored, &programs, Some(&registry));
        let after = construct(&programs[0], &authored, &programs, &registry);
        match (before, after) {
            (Err(old), Err(new)) => assert_eq!(old, new),
            (Ok(old), Ok(view)) => {
                let layout = registry.get(&programs[0], &authored, &programs).unwrap();
                let old = hir::resolve_with_function_reuse(
                    &old,
                    None,
                    None,
                    None,
                    layout.presentation(&programs[0], &old).unwrap(),
                )
                .err()
                .unwrap();
                let new =
                    hir::resolve_source(&view, layout.presentation(&programs[0], &view).unwrap())
                        .err()
                        .unwrap();
                assert_eq!(old, new);
            }
            _ => panic!("complete old/new constructor verdict differs"),
        }
    }
}

#[test]
fn complete_recursive_owner_receivers_pay_drop_cache_and_short_refusal() {
    for (caller, provider) in [
        (VEC_CALLER, VEC_PROVIDER),
        (BOX_CALLER, BOX_PROVIDER),
        (CLASS_CALLER, CLASS_PROVIDER),
        (INVARIANT_CALLER, INVARIANT_PROVIDER),
    ] {
        let programs = sources(caller, provider);
        let authored = index_authored(&programs).unwrap();
        let observe = |limit, retain| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let registry = LayoutRegistry::new(&programs, &authored);
                    let result = (|| {
                        let costs =
                            super::super::super::synthetic_builder_bytes_scoped_with_registry(
                                &programs[0],
                                &authored,
                                &programs,
                                None,
                                4,
                                true,
                                Some(&registry),
                            )?;
                        reserve_builder_structure(costs.raw_clone_and_hir)?;
                        let view = construct(&programs[0], &authored, &programs, &registry)?;
                        let layout = registry.get(&programs[0], &authored, &programs)?;
                        let resolved = hir::identity_resolution::resolve_source(
                            &view,
                            layout.presentation(&programs[0], &view)?,
                            |bytes| reserve_builder_structure(bytes),
                        )
                        .map_err(|error| match error {
                            hir::identity_resolution::Failure::Source(errors) => errors,
                            hir::identity_resolution::Failure::Semantic(error) => vec![error],
                            hir::identity_resolution::Failure::Identity(errors) => errors,
                        })?;
                        hir::validate(&resolved.0).map_err(|error| vec![error])?;
                        drop(resolved);
                        if retain {
                            drop(
                                super::super::super::borrowed_leaf::Input::Borrowed(view)
                                    .into_checked_key()?,
                            );
                        } else {
                            drop(view);
                        }
                        Ok::<(), Vec<Diagnostic>>(())
                    })();
                    if result.is_err() {
                        assert!(reserve_builder_structure(0).is_err());
                    }
                    drop(result);
                    drop(registry);
                })
            })
        };
        for retain in [false, true] {
            let (((), observed), refused, required, _) = observe(usize::MAX, retain);
            assert!(!refused && !observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
            assert!(observed.allocated <= required && observed.highwater <= required);
            let (((), observed), refused, paid, _) = observe(required, retain);
            assert!(!refused && !observed.invalid && observed.live == 0 && paid == required);
            let (((), observed), refused, _, first) = observe(required - 1, retain);
            assert!(refused && first.is_some() && !observed.invalid && observed.live == 0);
        }
    }
}

#[test]
fn borrowed_authored_invariant_transform_matches_the_complete_owned_route() {
    let source = r#"module invariant_leaf;
@id("leaf.row") record Row { @id("leaf.row.x") x:i64, } requires x>=0
@id("leaf.main") fn main()->i64 {let mut row=Row{x:41}; row.x=42; row.x}"#;
    let program = crate::parse(source, "invariant-leaf.spx").unwrap();
    let view = super::super::super::borrowed_leaf::construct(&program).unwrap();
    let old = hir::resolve_with_function_reuse(&program, None, None, None, None).unwrap();
    let new = hir::resolve_source(&view, None).unwrap();
    assert_eq!(old.0, new.0);
    assert_eq!(old.1.costs(), new.1.costs());
    assert!(std::ptr::eq(
        view.types().get(0).unwrap(),
        &program.types[0]
    ));
}

#[test]
fn literal_box_wrapper_overlay_removes_real_caller_copy_and_function_growth() {
    let programs = sources(BOX_CALLER, BOX_PROVIDER);
    let authored = index_authored(&programs).unwrap();
    let observe = |borrow| {
        measure(|| {
            let registry = LayoutRegistry::new(&programs, &authored);
            if borrow {
                drop(construct(&programs[0], &authored, &programs, &registry).unwrap());
            } else {
                drop(old::construct(&programs[0], &authored, &programs, Some(&registry)).unwrap());
            }
            drop(registry);
        })
    };
    let ((), before) = observe(false);
    let ((), after) = observe(true);
    // Independently saved producer: one authored function directory (capacity1)
    // followed by two imported wrappers grows to capacity4. Generated-only
    // storage acquires capacity2. The authored main is one Block tail containing
    // two one-argument calls, each with one explicit scalar type argument.
    assert!(std::mem::size_of::<crate::ast::Function>() <= 1024);
    let directories = 3 * std::mem::size_of::<crate::ast::Function>();
    let main_payload = 3 * std::mem::size_of::<crate::ast::Expr>()
        + 2 * std::mem::size_of::<crate::ast::Type>()
        + "overlay.box.main".len()
        + "main".len()
        + "consume".len()
        + "allocate".len();
    let sidecars = 2 * std::mem::size_of::<crate::ast::ModuleUse>()
        + "std.mem.box.new".len()
        + "std.mem.box.into-inner".len()
        + 2 * "std.mem".len()
        + "allocate".len()
        + "consume".len()
        + "caller.spx".len()
        + "overlay.box".len();
    // Both iterative receivers obtain Expr capacity4. The new receiver has
    // four signature roots plus two wrapper-call type arguments (six Types:
    // capacities4,8). The full old receiver adds the main result and two call
    // arguments (nine Types: capacities4,8,16), so only its real capacity16
    // replacement and one additional realloc are removed.
    let drop_replacement = 16 * std::mem::size_of::<crate::ast::Type>();
    assert_eq!(
        before.allocated,
        after.allocated + directories + main_payload + sidecars + drop_replacement
    );
    assert_eq!(before.reallocations, after.reallocations + 2);
    for observed in [before, after] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn conservative_class_default_census_keeps_real_owned_record_forwarding() {
    let caller=super::CALLER.replacen("module function_layout.app;",
        "module function_layout.app; use type @id(\"overlay.unused_class\") from function_layout.provider as Unused;",1);
    let provider=format!("{} @id(\"overlay.unused_class\") class Unused {{ @id(\"overlay.unused_class.x\") x:i64, }}",super::PROVIDER);
    parity(&caller, &provider);
    let programs = sources(&caller, &provider);
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let view = construct(&programs[0], &authored, &programs, &registry).unwrap();
    let forward = view
        .functions()
        .iter()
        .find(|function| function.name == "forward")
        .unwrap();
    // A class import enables iterative ownership. The census deliberately
    // overbounds the record default tree; the unchanged actual provider proof
    // still forwards exactly one existing owner through a Var expression.
    assert!(matches!(&forward.body.kind, crate::ast::ExprKind::Var(_)));
}
