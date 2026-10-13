//! Whole original verifier/type-table receivers and constructor lifetime controls.
use super::*;
use crate::ast::TypeDeclaration;
use crate::source_verify::private_names::{PrivateNames, Providers};
use crate::test_allocation_observer::measure;
#[rustfmt::skip]
mod corpus;
fn json(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics.iter().map(Diagnostic::json).collect()
}
fn verify(program: &dyn ProgramSource) -> Vec<Diagnostic> {
    let current = super::verify(program);
    let old = declaration::old::verify(program);
    assert_eq!(json(&current), json(&old));
    assert_eq!(
        current.first().map(Diagnostic::json),
        old.first().map(Diagnostic::json)
    );
    current
}
fn compare_public_source(source: &str) {
    let program = crate::parse(source, "constructor-full-public.spx").unwrap();
    let before = program.clone();
    let _ = verify(&program);
    assert_eq!(program, before);
}
const GENERIC: &str = r#"module constructor.generic;
@id("packet") record Packet { @id("packet.n") n:i64, }
@id("identity") fn identity<T>(value:T)->T {value}
@id("relay") fn relay<A,B>(left:A,right:B)->A {identity(left)}
@id("probe") fn probe(n:i64)->i64 requires n>=0 ensures result>=0 {identity(n)}
@id("main") fn main()->i64 {relay(1,true)}
"#;
const FIELDS: &str = r#"module constructor.fields;
@id("packet.a") record Packet { @id("packet.x.a") x:i64, @id("packet.x.b") x:bool, }
@id("packet.b") record Packet { @id("packet.x.c") x:bool, @id("packet.y") y:i64, }
@id("cycle.a") class A:B { @id("a.x") x:i64, }
@id("cycle.b") class B:A { @id("b.x") x:bool, }
@id("main") fn main()->i64 {0}
"#;
#[test]
fn complete_old_public_pass_preserves_generic_duplicate_namespace_cycle_and_first_json() {
    for source in [
        GENERIC,
        FIELDS,
        "module t; fn same(x:i64)->i64 {x} fn same<T>(x:T)->T {x} fn main()->i64 {same(1)}",
        "module t; fn recursive<T>(x:T)->T {recursive(x)} fn main()->i64 {0}",
        "module t; fn probe(n:i64,n:bool)->i64 requires n ensures n {0} fn main()->i64 {0}",
        "module t; fn main()->i64 {let x=1; let x=missing; x}",
        "module t; fn main()->i64 {let values=vec_with_capacity<i64>(1usize);vec_len<i64>(values)}",
    ] {
        compare_public_source(source);
    }
    let program = crate::parse(GENERIC, "constructor-partition.spx").unwrap();
    assert_eq!(
        json(&super::verify_partition(&program)),
        json(&declaration::old::verify_partition(&program))
    );
}
#[test]
fn borrowed_field_lookup_keeps_last_field_and_winning_declaration_including_prelude() {
    let mut program = crate::parse(FIELDS, "constructor-fields.spx").unwrap();
    let current = TypeTable::new(&program);
    let old = TypeTable::new_owned_reference(&program);
    for declaration in program
        .types
        .iter()
        .chain(crate::prelude::declarations_for_program(&program))
    {
        let winner = current.declaration(&declaration.name);
        assert_eq!(
            winner.map(|v| v as *const _),
            old.declaration(&declaration.name).map(|v| v as *const _)
        );
        for name in ["x", "y", "missing"] {
            assert_eq!(
                current
                    .declared_field(&declaration.name, name)
                    .map(|v| v as *const _),
                old.declared_field(&declaration.name, name)
                    .map(|v| v as *const _)
            );
        }
    }
    let winner = &program.types[1];
    let TypeDeclarationKind::Record { fields } = &winner.kind else {
        panic!("record");
    };
    assert!(std::ptr::eq(
        current.declared_field("Packet", "x").unwrap(),
        &fields[0]
    ));
    assert_eq!(current.merged_class_fields, old.merged_class_fields);
    assert_eq!(current.class_parents, old.class_parents);
    drop(current);
    drop(old);
    program.types.truncate(1);
    let current = TypeTable::new(&program);
    let TypeDeclarationKind::Record { fields } = &program.types[0].kind else {
        panic!("record");
    };
    assert!(std::ptr::eq(
        current.declared_field("Packet", "x").unwrap(),
        &fields[1]
    ));
    drop(current);
    let mut collision = crate::parse(
        "module t; fn main()->i64 {let values=vec_with_capacity<i64>(1usize);0}",
        "constructor-prelude.spx",
    )
    .unwrap();
    let mut authored = program.types[0].clone();
    authored.name = "Vec".to_owned();
    collision.types.push(authored);
    let current = TypeTable::new(&collision);
    let old = TypeTable::new_owned_reference(&collision);
    assert_eq!(
        current.declaration("Vec").map(|v| v as *const _),
        old.declaration("Vec").map(|v| v as *const _)
    );
    assert!(!std::ptr::eq(
        current.declaration("Vec").unwrap(),
        &collision.types[0]
    ));
}
/// A held closure whose single copy is the last declaration of the program.
struct Held<'a>(&'a TypeDeclaration);
impl Providers for Held<'_> {
    fn provider(&self, index: usize) -> Option<(&str, &TypeDeclaration)> {
        (index == 0).then_some(("held.provider", self.0))
    }
}

const PROVIDER: &str = "module held.provider;

@id(\"copy\")
record Decoded {
    @id(\"copy.n\")
    n: i64,
}

@id(\"zero\")
fn zero() -> i64
{
    0
}
";

// `_spx_layout_0` is an authored declaration below the bound copies; it keeps
// its authored spelling even though it shares the private prefix.
const SYNTHETIC: &str = "module held.caller;

@id(\"authored\")
record _spx_layout_0 {
    @id(\"authored.n\")
    n: i64,
}

@id(\"copy\")
record _spx_layout_1 {
    @id(\"copy.n\")
    n: i64,
}

@id(\"pick\")
fn pick(value: _spx_layout_1, other: _spx_layout_0) -> i64
{
    if value.n == 0 { value } else { other }
}
";

const PRESENTED: &str =
    "`if` branches return different types: Decoded [unimported @id(\"copy\") from held.provider] \
     and _spx_layout_0";
const ORDINARY: &str = "`if` branches return different types: _spx_layout_1 and _spx_layout_0";

#[test]
fn complete_old_private_presentation_keeps_exact_subject_provider_and_authored_spelling() {
    let provider = crate::parse(PROVIDER, "provider.spx").unwrap();
    let program = crate::parse(SYNTHETIC, "caller.spx").unwrap();
    let held = Held(&provider.types[0]);
    let view = PrivateNames::new(&program, &program, program.types.len() - 1, &held);
    let current = super::verify_presenting(&program, Some(view));
    let old = declaration::old::verify_presenting(&program, Some(view));
    assert_eq!(json(&current), json(&old));
    assert_eq!(
        current
            .iter()
            .find(|d| d.code == "SPX-T211")
            .unwrap()
            .message,
        PRESENTED
    );
    let mut generic_private = program.clone();
    let mut generic_program = crate::parse(GENERIC, "generic-private.spx").unwrap();
    generic_private
        .functions
        .append(&mut generic_program.functions);
    let generic_view = PrivateNames::new(
        &generic_private,
        &generic_private,
        generic_private.types.len() - 1,
        &held,
    );
    assert_eq!(
        json(&super::verify_presenting(
            &generic_private,
            Some(generic_view)
        )),
        json(&declaration::old::verify_presenting(
            &generic_private,
            Some(generic_view)
        )),
    );
    let drifted = program.clone();
    let current = super::verify_presenting(&drifted, Some(view));
    let old = declaration::old::verify_presenting(&drifted, Some(view));
    assert_eq!(json(&current), json(&old));
    assert_eq!(
        current
            .iter()
            .find(|d| d.code == "SPX-T211")
            .unwrap()
            .message,
        ORDINARY
    );
    assert_eq!(
        json(&super::verify_presenting(&program, None)),
        json(&declaration::old::verify_presenting(&program, None))
    );
    assert_eq!(
        json(&super::verify(&program)),
        json(&declaration::old::verify(&program))
    );
}
#[test]
fn full_public_receiver_and_secondary_directories_observe_actual_requests_through_drop() {
    let program = crate::parse(GENERIC, "constructor-physical.spx").unwrap();
    let ((), current) = measure(|| {
        drop(super::verify(&program));
    });
    let ((), old) = measure(|| {
        drop(declaration::old::verify(&program));
    });
    assert!(!current.invalid && !old.invalid && current.live == 0 && old.live == 0);
    assert_eq!(current.allocated, current.freed);
    assert_eq!(old.allocated, old.freed);
    assert!(current.allocations < old.allocations && current.allocated < old.allocated);
    let ((), current) = measure(|| {
        drop(TypeTable::new(&program));
    });
    let ((), old) = measure(|| {
        drop(TypeTable::new_owned_reference(&program));
    });
    assert!(!current.invalid && !old.invalid && current.live == 0 && old.live == 0);
    assert_eq!(current.allocated, current.freed);
    assert_eq!(old.allocated, old.freed);
    assert!(current.allocations < old.allocations && current.allocated < old.allocated);
    // Ordinary verifier setup remains public; these are actual requested layouts,
    // not ledger attribution, budget discounts, RSS or application-fit evidence.
}
#[test]
fn requires_borrows_same_entry_state_and_body_ensures_keep_order_and_input_unchanged() {
    for source in [
        "module t; fn probe(flag:bool,n:i64)->i64 requires {let n=true;n} requires flag ensures result==n {n} fn main()->i64 {0}",
        "module t; fn inspect(x:own Bytes)->i64 requires {let local=bytes_zeroed(2usize);byte_len(bytes_as_slice(local))==2usize} ensures result==0 {byte_len(bytes_as_slice(x));0} fn main()->i64 {0}",
        "module t; fn probe(n:i64)->i64 requires missing ensures also_missing {body_missing} fn main()->i64 {0}",
    ] {
        compare_public_source(source);
    }
}
#[test]
fn complete_saved_pass_sources_and_all_children_remain_joined_and_within_caps() {
    let source = concat!(
        include_str!("../source_verify.rs"),
        include_str!("recovery.rs"),
        include_str!("recovery_legacy.rs"),
        include_str!("recovery_legacy/enter.rs"),
        include_str!("recovery_legacy/blocks.rs"),
        include_str!("recovery_legacy/operators.rs"),
        include_str!("recovery_legacy/records.rs"),
        include_str!("recovery_legacy/match_arms.rs"),
        include_str!("recovery_tests/refusal_dependency.rs"),
        include_str!("declaration.rs"),
        include_str!("declaration/functions.rs"),
        include_str!("declaration/old.rs"),
        include_str!("declaration/functions/old.rs"),
        include_str!("type_table.rs"),
        include_str!("type_table/native_containers.rs"),
        include_str!("type_table/old.rs"),
        include_str!("constructor_tests.rs"),
        include_str!("constructor_tests/corpus.rs")
    );
    assert!(source.contains("entry_variables = variables.clone()"));
    assert!(source.contains("complete old field index"));
    for part in [
        include_str!("declaration/functions/old.rs"),
        include_str!("declaration/old.rs"),
        include_str!("type_table/old.rs"),
        include_str!("constructor_tests/corpus.rs"),
    ] {
        assert!(part.lines().count() <= 1500);
    }
}
