//! Borrow discovery only. Final publication still owns the selected functions.
use super::*;
use crate::test_allocation_observer::measure;
#[path = "package_link_borrowed/old.rs"]
mod old;
fn fixture(unused: usize) -> WorkspaceGraphBuild {
    let mut text=String::from("module package.link; @id(\"pkg.leaf\") fn leaf(n:i64)->i64 {n+1} @id(\"pkg.main\") fn main()->i64 {leaf(41)}\n");
    for n in 0..unused {
        text.push_str(&format!(
            "@id(\"pkg.unused.{n}\") fn unused_{n}(n:i64)->i64 {{let a=n+1;let b=a+2;b+3}}\n"
        ));
    }
    build_owned(vec![canonical_source("package/link.spx", &text)]).unwrap()
}
fn compare(workspace: &WorkspaceGraphBuild, roots: &[String]) {
    let expected = old::link(workspace, "package.link", roots);
    let actual = workspace.linked_package_scalar_exports("package.link", roots);
    match (expected, actual) {
        (Ok(expected), Ok(actual)) => {
            hir::validate(&expected).unwrap();
            hir::validate(&actual).unwrap();
            assert_eq!(
                crate::cache_codec::encode(&actual).unwrap(),
                crate::cache_codec::encode(&expected).unwrap()
            );
        }
        (Err(expected), Err(actual)) => {
            assert_eq!(actual.len(), expected.len());
            for (a, b) in actual.iter().zip(&expected) {
                assert_eq!(a.code, b.code);
                assert_eq!(a.message, b.message);
            }
        }
        _ => panic!("owned and borrowed publication differ"),
    }
}
#[test]
fn borrowed_package_discovery_preserves_owned_wire_all_guards_and_first_error_order() {
    let roots = vec!["pkg.main".to_owned()];
    for count in [0, 8, 32] {
        compare(&fixture(count), &roots);
    }
    for mode in 0..5 {
        let mut workspace = fixture(2);
        match mode {
            0 => {
                workspace.hir.declarations.remove("pkg.main");
            }
            1 => {
                workspace.hir.declarations.get_mut("pkg.main").unwrap().path =
                    Some("foreign.spx".into());
            }
            2 => {
                workspace
                    .hir
                    .declarations
                    .get_mut("pkg.main")
                    .unwrap()
                    .origin = hir::IdentityOrigin::Automatic;
            }
            3 => {
                let duplicate = workspace.hir.modules[0]
                    .functions
                    .iter()
                    .find(|f| f.id.as_str() == "pkg.main")
                    .unwrap()
                    .clone();
                workspace.hir.modules[0].functions.push(duplicate);
            }
            _ => {
                workspace
                    .hir
                    .declarations
                    .get_mut("pkg.main")
                    .unwrap()
                    .module = Some("foreign.module".into());
                workspace.hir.declarations.remove("pkg.leaf");
            }
        }
        assert!(old::link(&workspace, "package.link", &roots).is_err());
        compare(&workspace, &roots);
    }
    for roots in [
        vec![],
        vec!["missing".into()],
        vec!["pkg.main".into(), "pkg.main".into()],
        vec!["pkg.main".into(), "pkg.leaf".into()],
    ] {
        compare(&fixture(2), &roots);
    }
}
#[test]
fn borrowed_package_discovery_removes_actual_unselected_hir_clones_through_publication_and_drop() {
    for count in [1, 8, 32] {
        let workspace = fixture(count);
        let roots = vec!["pkg.main".to_owned()];
        let (_, old) = measure(|| {
            let published = old::link(&workspace, "package.link", &roots).unwrap();
            assert_eq!(published.functions.len(), 2);
            drop(published);
        });
        let (_, borrowed) = measure(|| {
            let published = workspace
                .linked_package_scalar_exports("package.link", &roots)
                .unwrap();
            assert_eq!(published.functions.len(), 2);
            drop(published);
        });
        assert_eq!(old.allocated, old.freed);
        assert_eq!(borrowed.allocated, borrowed.freed);
        assert_eq!(old.live, 0);
        assert_eq!(borrowed.live, 0);
        // Independently unavoidable old body header backing for each unused
        // function's authored Block: two statements plus one boxed tail.
        let minimum = count
            * (2 * std::mem::size_of::<hir::ResolvedStatement>()
                + std::mem::size_of::<hir::ResolvedExpr>());
        assert!(old.allocated - borrowed.allocated >= minimum);
        assert!(old.allocations > borrowed.allocations);
    }
}
