//! Narrow directory construction observations; whole-validator funding is separate.
use super::*;
use crate::test_allocation_observer::{measure, Metrics};
use std::fmt::Write as _;
use std::mem::size_of;

type Owned<'a> = (
    BTreeSet<DeclarationId>,
    BTreeSet<DeclarationId>,
    BTreeSet<DeclarationId>,
    BTreeMap<DeclarationId, &'a ResolvedImport>,
);
type Borrowed<'a> = (Ids<'a>, Ids<'a>, Ids<'a>, Imports<'a>);
fn program() -> ResolvedProgram {
    let mut source = String::from("module test.identity.scratch;\n");
    for n in 0..64 {
        writeln!(source, "@id(\"token.{n}\") resource Token{n} {{ @id(\"token.{n}.drop\") drop import \"host.dispose.{n}\"; }}").unwrap();
    }
    source.push_str("@id(\"host\") interface Host permits {} {\n");
    for n in 0..64 {
        writeln!(source, "@id(\"host.dispose.{n}\") import fn dispose{n}(value:own Token{n})->unit effects {{}} failure infallible consumes value always;").unwrap();
    }
    source.push_str("}\n@id(\"app.main\") fn main()->i64 {0}\n");
    crate::hir::resolve(&crate::parse(&source, "identity-scratch.spx").unwrap()).unwrap()
}
// Saved original owning-key construction, including source-order insertions.
fn old(program: &ResolvedProgram) -> Owned<'_> {
    let types = program.types.iter().map(|item| item.id.clone()).collect();
    let mut interfaces = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut imports = BTreeMap::new();
    for interface in &program.interfaces {
        assert!(interfaces.insert(interface.id.clone()));
        for import in &interface.imports {
            assert!(ids.insert(import.id.clone()));
            assert!(imports.insert(import.id.clone(), import).is_none());
        }
    }
    (types, interfaces, ids, imports)
}
fn current(program: &ResolvedProgram) -> Borrowed<'_> {
    let types = program.types.iter().map(|item| &item.id).collect::<Ids>();
    let mut interfaces = Ids::new();
    let mut ids = Ids::new();
    let mut imports = Imports::new();
    for interface in &program.interfaces {
        assert!(interfaces.insert(&interface.id));
        for import in &interface.imports {
            assert!(ids.insert(&import.id));
            assert!(imports.insert(&import.id, import).is_none());
        }
    }
    (types, interfaces, ids, imports)
}
fn settled(metrics: Metrics) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(metrics.reallocations, 0);
}
#[test]
fn borrowed_identity_scratch_preserves_saved_text_order_and_removes_owning_key_storage_through_drop(
) {
    let program = program();
    crate::hir::validate(&program).unwrap();
    let expected = old(&program);
    let actual = current(&program);
    assert_eq!(
        actual.0.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        expected
            .0
            .iter()
            .map(DeclarationId::as_str)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        actual.1.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        expected
            .1
            .iter()
            .map(DeclarationId::as_str)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        actual.2.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        expected
            .2
            .iter()
            .map(DeclarationId::as_str)
            .collect::<Vec<_>>()
    );
    for interface in &program.interfaces {
        for import in &interface.imports {
            // A detached equal-text key still finds the exact current import.
            let detached = DeclarationId::new(import.id.as_str());
            assert!(std::ptr::eq(*expected.3.get(&detached).unwrap(), import));
            assert!(std::ptr::eq(*actual.3.get(&detached).unwrap(), import));
        }
    }
    assert_eq!(actual.3.len(), 64);
    assert_eq!(size_of::<DeclarationId>(), 3 * size_of::<usize>());
    assert_eq!(size_of::<&DeclarationId>(), size_of::<usize>());
    // Literal logical key census is distinct from private BTree node backing.
    // The current resolver appends the two exact compiler Vec/Box types.
    // They contribute key carriers even though their identity text is static.
    let mut authored = (0..64).map(|n| format!("token.{n}")).collect::<Vec<_>>();
    authored.extend([
        crate::prelude::VEC_ID.to_owned(),
        crate::prelude::BOX_ID.to_owned(),
    ]);
    authored.sort();
    assert_eq!(
        actual.0.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        authored.iter().map(String::as_str).collect::<Vec<_>>()
    );
    assert_eq!(actual.0.len(), 64 + 2);
    assert_eq!(actual.1.len(), 1);
    assert_eq!(actual.2.len(), 64);
    let keys = (64 + 2) + 1 + 64 + 64;
    assert_eq!(
        keys * (size_of::<DeclarationId>() - size_of::<&DeclarationId>()),
        390 * size_of::<usize>()
    );
    drop((actual, expected));
    let (_, owned_metrics) = measure(|| drop(old(&program)));
    let (_, borrowed_metrics) = measure(|| drop(current(&program)));
    settled(owned_metrics);
    settled(borrowed_metrics);
    assert_eq!(borrowed_metrics.allocations, owned_metrics.allocations);
    assert!(borrowed_metrics.allocated < owned_metrics.allocated);
}
fn error(program: &ResolvedProgram) -> Diagnostic {
    crate::hir::validate(program).unwrap_err()
}
#[test]
fn borrowed_identity_scratch_keeps_duplicate_and_current_index_first_error_order() {
    let original = program();
    crate::hir::validate(&original).unwrap();
    let mut program = original.clone();
    program.types.push(program.types[0].clone());
    program.interfaces.push(program.interfaces[0].clone());
    assert_eq!(
        error(&program).message,
        "duplicate resolved type declaration identity"
    );
    let mut program = original.clone();
    program.interfaces.push(program.interfaces[0].clone());
    assert_eq!(
        error(&program).message,
        "duplicate resolved interface identity `host`"
    );
    let mut program = original.clone();
    let duplicate = program.interfaces[0].imports[0].clone();
    program.interfaces[0].imports.push(duplicate);
    assert_eq!(
        error(&program).message,
        "duplicate resolved import identity `host.dispose.0`"
    );
    let mut program = original.clone();
    program.interfaces[0].imports[0].interface = DeclarationId::new("wrong.owner");
    assert_eq!(
        error(&program).message,
        "import `host.dispose.0` has an invalid owner or logical key"
    );
    let mut program = original.clone();
    program.interfaces[0].name.push_str(".forged");
    assert_eq!(
        error(&program).message,
        "resolved interface `host` disagrees with its declaration index entry"
    );
    crate::hir::validate(&original).unwrap();
}
