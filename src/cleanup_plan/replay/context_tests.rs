//! A borrowed context runs the complete direct replay without a Program clone.
use super::*;
struct Borrowed<'a>(&'a ResolvedProgram);
impl crate::cleanup::ReplayContext for Borrowed<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        &self.0.declarations
    }
    fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
        crate::hir::items::View::slice(&self.0.types)
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        &self.0.interfaces
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        crate::hir::items::View::slice(&self.0.functions)
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        &self.0.function_templates
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        &self.0.function_instances
    }
}
#[test]
fn borrowed_context_replays_lexical_renewal_and_rejects_stale_cleanup() {
    let source = crate::check(
        r#"module replay.borrowed;
@id("borrowed.run") fn run()->i64 {
 let key=str"x"; let mut output=bytes_zeroed(1usize); let mut at=0;
 while at<3 {
  output=bytes_set(output,0usize,65u8); at=at+1;
  if at==1 {continue;}; 0
 }
 return str_len_bytes(key);
}
@id("app.main") fn main()->i64 {run()}
"#,
        "borrowed-replay.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "borrowed.run")
        .unwrap();
    assert_eq!(
        function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
    );
    let borrowed = Borrowed(&program);
    assert!(borrowed.full_program().is_none());
    validate_structure(&program, function).unwrap();
    validate_structure(&borrowed, function).unwrap();
    let mut stale = function.clone();
    stale.cleanup_plan.lexical_control.as_mut().unwrap().loops[0].condition =
        function.body.id.clone();
    assert_eq!(
        validate_structure(&borrowed, &stale).unwrap_err().code,
        "SPX-H006"
    );
}

// A sparse local sequence must lend its actual selected members; it cannot be
// represented by an empty slice or a copied contiguous HIR projection.
struct Sparse<'a> {
    program: &'a ResolvedProgram,
    types: crate::hir::items::View<'a, crate::hir::ResolvedTypeDeclaration>,
    functions: crate::hir::items::View<'a, ResolvedFunction>,
}
impl crate::cleanup::ReplayContext for Sparse<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        &self.program.declarations
    }
    fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
        self.types
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        self.functions
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        &self.program.interfaces
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        &self.program.function_templates
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        &self.program.function_instances
    }
}
#[test]
fn sparse_local_replay_sequences_lend_exact_current_members_without_materialization() {
    use crate::cleanup::ReplayContext;
    let source = crate::check(
        r#"module replay.sparse;
@id("sparse.A") record A { value:i64 }
@id("sparse.B") record B { value:i64 }
@id("sparse.C") record C { value:i64 }
@id("sparse.first") fn first()->i64 {1}
@id("sparse.omitted") fn omitted()->i64 {2}
@id("sparse.main") fn main()->i64 {3}
"#,
        "sparse-replay.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let type_ordinals = ["sparse.C", "sparse.A"].map(|id| {
        program
            .types
            .iter()
            .position(|item| item.id.as_str() == id)
            .unwrap()
    });
    let function_ordinals = [2usize, 0];
    let context = Sparse {
        program: &program,
        types: crate::hir::items::View::indexed(&program.types, &type_ordinals),
        functions: crate::hir::items::View::indexed(&program.functions, &function_ordinals),
    };
    let (_, observed) = crate::test_allocation_observer::measure(move || {
        let context = context;
        assert_eq!((context.type_count(), context.function_count()), (2, 2));
        for ordinal in 0..2 {
            assert!(std::ptr::eq(
                context.type_at(ordinal).unwrap(),
                &program.types[type_ordinals[ordinal]]
            ));
            assert!(std::ptr::eq(
                context.function_at(ordinal).unwrap(),
                &program.functions[function_ordinals[ordinal]]
            ));
        }
        assert!(context.type_at(2).is_none());
        assert!(context.function_at(2).is_none());
        assert!(context
            .resolve_call_target(&program.functions[1].id, None)
            .is_none());
        assert!(std::ptr::eq(
            context
                .resolve_call_target(&program.functions[0].id, None)
                .unwrap(),
            &program.functions[0]
        ));
        assert!(std::ptr::eq(
            context.functions().iter().next().unwrap(),
            &program.functions[2]
        ));
        assert!(std::ptr::eq(
            context.types().iter().next().unwrap(),
            &program.types[type_ordinals[0]]
        ));
    });
    assert_eq!(
        (
            observed.allocations,
            observed.reallocations,
            observed.allocated,
            observed.live
        ),
        (0, 0, 0, 0)
    );
    assert!(!observed.invalid);
}
