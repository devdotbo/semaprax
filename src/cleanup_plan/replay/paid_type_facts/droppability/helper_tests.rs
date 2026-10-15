//! Isolated helper allocation controls: no BTree acquisition can mask scratch.
use super::super::ReplayBudget;
use super::tests::parity;
use crate::bounded_output::with_limit_usage_refusal;
use crate::hir::{CompilerDeclaration, DeclarationId, ResolvedProgram, ResolvedType};
use crate::test_allocation_observer::measure;

const SOURCE: &str = r#"module droppability.helpers;
@id("helper.owned") record Owned {
 @id("helper.owned.a") a: Bytes,
 @id("helper.owned.b") b: Bytes,
 @id("helper.owned.count") count: i64,
}
@id("helper.leaf") record Leaf {
 @id("helper.leaf.text") text: string,
 @id("helper.leaf.flag") flag: bool,
}
@id("helper.copy") record Copy {
 @id("helper.copy.count") count: i64,
 @id("helper.copy.flag") flag: bool,
}
@id("helper.generic") record Generic<T> {
 @id("helper.generic.payload") payload: T,
}
@id("helper.variant") variant Choice<T> {
 @id("helper.variant.none") None,
 @id("helper.variant.some") Some { @id("helper.variant.payload") payload: T, },
}
@id("helper.consume.owned") fn consume_owned(rows: own Vec<Owned>) -> i64 { 0 }
@id("helper.consume.leaf") fn consume_leaf(rows: own Vec<Leaf>) -> i64 { 0 }
@id("helper.consume.copy") fn consume_copy(rows: own Vec<Copy>) -> i64 { 0 }
@id("helper.main") fn main() -> i64 { 0 }
"#;
fn fixture() -> ResolvedProgram {
    let ast = crate::check(SOURCE, "droppability-helpers.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
fn authored(program: &ResolvedProgram, name: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: program.declarations.type_id(name).unwrap().clone(),
        arguments,
    }
}
fn compiler(id: CompilerDeclaration, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::compiler(id),
        arguments,
    }
}
fn probes(program: &ResolvedProgram, ty: &ResolvedType) -> [bool; 11] {
    let index = &program.declarations;
    [
        crate::map_ops::is_typed_collection(ty),
        crate::stdin_stream_ops::is_reader(ty),
        crate::iterator_ops::resolved_element_is_admitted_in(index, ty),
        crate::hir::owned_leaf_collection::copy_or_leaf_admitted(index, ty),
        crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(index, ty),
        crate::vec_ops::resolved_vec_element_is_admitted(ty),
        crate::box_ops::resolved_box_element_is_admitted(ty),
        crate::hir::declared_collection::text_element(index, ty),
        crate::hir::copy_record_collection::admitted(index, ty),
        crate::hir::owned_leaf_collection::layout(index, ty).is_some(),
        crate::hir::owned_leaf_collection::runtime_element(index, ty),
    ]
}
#[test]
fn real_admission_helpers_have_zero_backing_at_zero_budget_without_tree_payment() {
    let program = fixture();
    // Real explicit source declaration/field/parameter IDs, not function IDs
    // masquerading as nominal declarations or forged cache-only fixtures.
    let cases = [
        (
            authored(&program, "Owned", vec![]),
            [
                false, false, true, true, true, false, false, false, false, true, true,
            ],
        ),
        (
            authored(&program, "Leaf", vec![]),
            [
                false, false, true, true, false, false, false, true, false, true, true,
            ],
        ),
        (
            authored(&program, "Copy", vec![]),
            [
                false, false, false, true, false, false, false, false, true, false, false,
            ],
        ),
        (
            authored(&program, "Generic", vec![ResolvedType::Bytes]),
            [false; 11],
        ),
        (
            authored(&program, "Choice", vec![ResolvedType::Bytes]),
            [false; 11],
        ),
        (
            authored(&program, "Choice", vec![ResolvedType::String]),
            [false; 11],
        ),
        (authored(&program, "Choice", vec![]), [false; 11]),
        (
            authored(
                &program,
                "Choice",
                vec![ResolvedType::Bytes, ResolvedType::Bool],
            ),
            [false; 11],
        ),
        (
            authored(&program, "Owned", vec![ResolvedType::Bytes]),
            [false; 11],
        ),
        (
            ResolvedType::String,
            [
                false, false, true, true, false, false, false, true, false, true, true,
            ],
        ),
        (
            ResolvedType::Bytes,
            [
                false, false, true, false, false, true, true, false, false, false, false,
            ],
        ),
        (
            ResolvedType::I64,
            [
                false, false, true, false, false, true, true, false, false, false, false,
            ],
        ),
        (
            compiler(
                CompilerDeclaration::Map,
                vec![ResolvedType::String, ResolvedType::Bool],
            ),
            [
                true, false, false, false, false, false, false, false, false, false, false,
            ],
        ),
        (
            compiler(CompilerDeclaration::Set, vec![ResolvedType::I64]),
            [
                true, false, false, false, false, false, false, false, false, false, false,
            ],
        ),
        (
            compiler(
                CompilerDeclaration::Map,
                vec![ResolvedType::Bytes, ResolvedType::Bool],
            ),
            [false; 11],
        ),
        (
            compiler(CompilerDeclaration::Map, vec![ResolvedType::String]),
            [false; 11],
        ),
        (
            compiler(CompilerDeclaration::StdinReader, vec![]),
            [
                false, true, false, false, false, false, false, false, false, false, false,
            ],
        ),
        (
            compiler(CompilerDeclaration::StdinReader, vec![ResolvedType::Bytes]),
            [false; 11],
        ),
    ];
    for (ty, expected) in cases {
        let (((), metrics), refused, charged, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert_eq!(probes(&program, &ty), expected);
            })
        });
        assert!(!refused && charged == 0 && first.is_none());
        assert!(
            !metrics.invalid
                && metrics.allocations == 0
                && metrics.allocated == 0
                && metrics.live == 0
        );
    }
}
#[test]
fn real_declaration_identity_clone_and_owned_diagnostic_receiving_have_zero_new_backing() {
    let program = fixture();
    for id in program.declarations.declarations().map(|item| &item.id) {
        let pointer = id.as_str().as_ptr();
        let (((), metrics), refused, charged, _) = with_limit_usage_refusal(0, || {
            measure(|| {
                let cloned = id.clone();
                assert_eq!(pointer, cloned.as_str().as_ptr());
                assert_eq!(&cloned, id);
                drop(cloned);
            })
        });
        assert!(!refused && charged == 0 && !metrics.invalid && metrics.allocations == 0);
    }
    let text = "already paid owned diagnostic carrier".to_owned();
    let pointer = text.as_ptr();
    let capacity = text.capacity();
    let ((diagnostic, metrics), refused, charged, _) = with_limit_usage_refusal(0, || {
        measure(|| crate::diagnostic::Diagnostic::io("SPX-H006", text))
    });
    assert_eq!(pointer, diagnostic.message.as_ptr());
    assert_eq!(capacity, diagnostic.message_owned_capacity());
    assert!(
        !refused
            && charged == 0
            && !metrics.invalid
            && metrics.allocations == 0
            && metrics.live == 0
    );
    drop(diagnostic);
}
#[test]
fn real_vec_iter_and_generic_refusals_keep_full_receivers_and_actual_funding() {
    let program = fixture();
    let mut roots: Vec<_> = program
        .functions
        .iter()
        .flat_map(|function| function.params.iter().map(|parameter| parameter.ty.clone()))
        .collect();
    assert_eq!(roots.len(), 3, "actual checked Vec signature roots");
    for name in ["Owned", "Leaf", "Copy"] {
        let element = authored(&program, name, vec![]);
        roots.push(compiler(CompilerDeclaration::Vec, vec![element.clone()]));
        if name != "Copy" {
            roots.push(compiler(CompilerDeclaration::Iter, vec![element]));
        }
    }
    roots.extend([
        compiler(CompilerDeclaration::Vec, vec![ResolvedType::String]),
        compiler(
            CompilerDeclaration::Vec,
            vec![authored(&program, "Generic", vec![ResolvedType::Bytes])],
        ),
        compiler(
            CompilerDeclaration::Vec,
            vec![authored(&program, "Choice", vec![ResolvedType::String])],
        ),
        compiler(
            CompilerDeclaration::Vec,
            vec![authored(&program, "Choice", vec![])],
        ),
        compiler(
            CompilerDeclaration::Iter,
            vec![authored(
                &program,
                "Choice",
                vec![ResolvedType::Bytes, ResolvedType::Bool],
            )],
        ),
        authored(&program, "Generic", vec![ResolvedType::Bytes]),
        authored(&program, "Choice", vec![ResolvedType::Bytes]),
        authored(&program, "Choice", vec![ResolvedType::String]),
        authored(&program, "Choice", vec![]),
        compiler(
            CompilerDeclaration::Map,
            vec![ResolvedType::String, ResolvedType::Bool],
        ),
        compiler(CompilerDeclaration::Set, vec![ResolvedType::I64]),
        compiler(CompilerDeclaration::StdinReader, vec![]),
    ]);
    for ty in roots {
        parity(&program, &ty);
        let function = &program.functions[0];
        let expected = crate::cleanup::type_needs_resource_cleanup(&program, &ty)
            .map_err(|message| super::super::replay_error(function, message));
        let observe = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = super::super::needs_drop(&program, function, &ty, &mut budget);
                    if crate::bounded_output::active_refused() {
                        assert!(result.is_err());
                        let before = crate::bounded_output::active_remaining();
                        let start = crate::test_allocation_observer::current_metrics();
                        drop(super::super::needs_drop(
                            &program,
                            function,
                            &ty,
                            &mut budget,
                        ));
                        let end = crate::test_allocation_observer::current_metrics();
                        assert_eq!(before, crate::bounded_output::active_remaining());
                        assert_eq!(start.allocated, end.allocated);
                        assert_eq!(start.allocations, end.allocations);
                    } else {
                        assert_eq!(result, expected);
                    }
                    drop(result);
                    drop(budget);
                })
            })
        };
        let (((), wide), refused, acquired, _) = observe(usize::MAX);
        assert!(
            !refused
                && acquired > 0
                && !wide.invalid
                && wide.live == 0
                && wide.allocated == wide.freed
        );
        assert!(wide.allocated <= acquired && wide.highwater <= acquired);
        let (((), exact), refused, used, _) = observe(acquired);
        assert!(
            !refused
                && used == acquired
                && !exact.invalid
                && exact.live == 0
                && exact.allocated == exact.freed
        );
        let (((), short), refused, _, first) = observe(acquired - 1);
        assert!(
            refused
                && first.is_some()
                && !short.invalid
                && short.live == 0
                && short.allocated == short.freed
        );
        let (((), zero), refused, used, first) = observe(0);
        assert!(refused && first.is_some() && used == 0 && zero.allocated == 0 && zero.live == 0);
    }
}
