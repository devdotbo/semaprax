use super::super::{Inventory, capture, legacy};
use super::{Directory, pure};
use crate::hir::{OwnershipMode, ResolvedType};
use crate::workspace_graph::{WorkspaceSource, index_authored};

fn source(count: usize) -> String {
    let mut text = String::from("module helpers;");
    for n in (0..count).rev() {
        text.push_str(&format!(
            "@id(\"helpers.f{n:04}\") fn f{n:04}(x:i64)->i64 {{x+1}}"
        ));
    }
    text.push_str("@id(\"helpers.main\") fn main()->i64 {f0000(41)}");
    text
}
fn programs(text: &str) -> Vec<crate::ast::Program> {
    vec![crate::check(text, "helpers.spx").unwrap(), crate::parse(
        "module caller; use function @id(\"helpers.f0000\") from helpers as helper; @id(\"caller.main\") fn main()->i64 {0}",
        std::path::Path::new("caller.spx")).unwrap()]
}
fn equal(a: &Inventory, b: &Inventory) {
    assert_eq!(a.revision, b.revision);
    assert_eq!(a.functions.len(), b.functions.len());
    for (a, b) in a.functions.iter().zip(&b.functions) {
        assert_eq!(
            (&a.id, a.explicit, &a.name, &a.result, a.digest),
            (&b.id, b.explicit, &b.name, &b.result, b.digest)
        );
        assert_eq!(a.params.len(), b.params.len());
        for (a, b) in a.params.iter().zip(&b.params) {
            assert_eq!(
                (&a.id, &a.name, &a.ty, a.ownership),
                (&b.id, &b.name, &b.ty, b.ownership)
            );
        }
    }
}
#[test]
fn old_capture_and_single_classification_keep_exact_inventory_and_digests() {
    for count in [1, 8, 64] {
        let text = source(count);
        let programs = programs(&text);
        let resolved = crate::hir::resolve(&programs[0]).unwrap();
        let authored = index_authored(&programs).unwrap();
        let sources = [WorkspaceSource {
            path: "helpers.spx".into(),
            source: text,
        }];
        let ((new, old), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            (
                capture(
                    &programs[0],
                    &resolved,
                    &programs,
                    &authored,
                    Some(&sources),
                )
                .unwrap()
                .unwrap(),
                legacy::capture(
                    &programs[0],
                    &resolved,
                    &programs,
                    &authored,
                    Some(&sources),
                )
                .unwrap()
                .unwrap(),
            )
        });
        assert!(!refused);
        equal(&new, &old);
        if count == 64 {
            assert!(new.capture_work < old.capture_work);
        }
    }
    // Dense checked bodies and zero-literal forwarding both retain original facts.
    for dense in [true, false] {
        let body = if dense {
            std::iter::repeat_n("x", 32).collect::<Vec<_>>().join("+")
        } else {
            "x".into()
        };
        let text = source(8).replace("x+1", &body);
        let programs = programs(&text);
        let resolved = crate::hir::resolve(&programs[0]).unwrap();
        let authored = index_authored(&programs).unwrap();
        let sources = [WorkspaceSource {
            path: "helpers.spx".into(),
            source: text,
        }];
        let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let new = capture(
                &programs[0],
                &resolved,
                &programs,
                &authored,
                Some(&sources),
            )
            .unwrap()
            .unwrap();
            let old = legacy::capture(
                &programs[0],
                &resolved,
                &programs,
                &authored,
                Some(&sources),
            )
            .unwrap()
            .unwrap();
            equal(&new, &old);
        });
        assert!(!refused);
    }
}
#[test]
fn wide_directory_classifies_once_with_exact_work_backing_and_allocation_free_drop() {
    let programs = programs(&source(256));
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let mut calls = 0;
    let ((units, bytes), refused, used) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            let mut work = usize::MAX;
            let directory = Directory::classify(&programs[0], &resolved, &mut work, &mut |f, w| {
                calls += 1;
                pure(f, w)
            })
            .unwrap();
            assert_eq!(directory.count(), programs[0].functions.len());
            for (n, f) in programs[0].functions.iter().enumerate() {
                assert_eq!(directory.selected(n).unwrap().id.as_str(), f.stable_id);
            }
            let bytes = std::mem::size_of::<super::Index<'_>>()
                + resolved.functions.len() * std::mem::size_of::<&crate::hir::ResolvedFunction>()
                + std::mem::size_of::<Directory<'_>>()
                + programs[0].functions.len()
                    * std::mem::size_of::<Option<&crate::hir::ResolvedFunction>>();
            drop(directory);
            (usize::MAX - work, bytes)
        });
    assert!(!refused);
    assert_eq!(calls, programs[0].functions.len());
    assert_eq!(used, bytes);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let mut work = units;
            drop(Directory::new(&programs[0], &resolved, &mut work).unwrap());
        });
        assert_eq!(observed.allocations, 2);
        assert_eq!(
            observed.allocated,
            bytes - std::mem::size_of::<super::Index<'_>>() - std::mem::size_of::<Directory<'_>>()
        );
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    });
    assert!(!refused);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = units;
        let directory = Directory::new(&programs[0], &resolved, &mut work).unwrap();
        let ((), observed) = crate::test_allocation_observer::measure(|| drop(directory));
        assert_eq!(observed.allocated, 0);
    });
    assert!(!refused);
    let ((), refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = units;
        drop(Directory::new(&programs[0], &resolved, &mut work).unwrap());
        assert_eq!(work, 0);
    });
    assert!(!refused);
    assert_eq!(used, bytes);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(bytes - 1, || {
        let mut work = units;
        let result = Directory::new(&programs[0], &resolved, &mut work);
        assert_eq!(work, 0);
        result.is_err()
    });
    assert!(short && refused);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = units - 1;
        let result = Directory::new(&programs[0], &resolved, &mut work);
        assert_eq!(work, 0);
        result.is_err()
    });
    assert!(short && !refused);
    // Drop never refunds sticky cumulative backing; a second directory still pays.
    let (short, refused, _) = crate::bounded_output::with_limit_usage(2 * bytes - 1, || {
        let mut work = usize::MAX;
        drop(Directory::new(&programs[0], &resolved, &mut work).unwrap());
        Directory::new(&programs[0], &resolved, &mut work).is_err()
    });
    assert!(short && refused);
}
#[test]
fn duplicate_origin_signature_mode_and_body_drift_do_not_gain_capture_authority() {
    let text = source(8);
    let programs = programs(&text);
    let authored = index_authored(&programs).unwrap();
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let sources = [WorkspaceSource {
        path: "helpers.spx".into(),
        source: text,
    }];
    for mutation in 0..4 {
        let mut changed = resolved.clone();
        match mutation {
            0 => changed.functions.push(changed.functions[0].clone()),
            1 => changed.functions[0].name.push('x'),
            2 => changed.functions[0].params[0].ownership = OwnershipMode::Borrow,
            3 => changed.functions[0].params[0].ty = ResolvedType::Bool,
            _ => unreachable!(),
        }
        let (bad, _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            match capture(&programs[0], &changed, &programs, &authored, Some(&sources)) {
                Err(_) | Ok(None) => true,
                Ok(Some(inventory)) => !inventory
                    .functions
                    .iter()
                    .any(|fact| fact.id == changed.functions[0].id),
            }
        });
        assert!(bad);
    }
    let cloned = programs[0].clone();
    let (bad, _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        capture(&cloned, &resolved, &programs, &authored, Some(&sources)).is_err()
    });
    assert!(bad);
    let (inventory, _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        capture(
            &programs[0],
            &resolved,
            &programs,
            &authored,
            Some(&sources),
        )
        .unwrap()
        .unwrap()
    });
    let mut changed = resolved.clone();
    changed.functions[0].body = changed.functions[1].body.clone();
    assert!(!super::super::matches_metered(
        &inventory,
        &inventory.functions[0],
        &changed.functions[0],
        &mut |_| Some(())
    ));
}

#[test]
fn real_utf8_provider_matches_original_capture_without_literal_inventory() {
    let text = format!(
        "{}\n@id(\"helper.entry\") fn main()->i64 {{0}}",
        include_str!("../../../../../std/data-json-utf8/src/utf8.spx")
    );
    let provider = crate::check(&text, "utf8.spx").unwrap();
    let caller = crate::parse("module helper.caller; use function @id(\"std.data.json.utf8.utf8_end\") from std.data.json.utf8 as end; @id(\"helper.caller.main\") fn main()->i64 {0}",std::path::Path::new("caller.spx")).unwrap();
    let programs = [provider, caller];
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let authored = index_authored(&programs).unwrap();
    let sources = [WorkspaceSource {
        path: "utf8.spx".into(),
        source: text,
    }];
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let new = capture(
            &programs[0],
            &resolved,
            &programs,
            &authored,
            Some(&sources),
        )
        .unwrap()
        .unwrap();
        let old = legacy::capture(
            &programs[0],
            &resolved,
            &programs,
            &authored,
            Some(&sources),
        )
        .unwrap()
        .unwrap();
        equal(&new, &old);
    });
    assert!(!refused);
}
