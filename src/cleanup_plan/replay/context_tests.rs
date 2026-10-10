//! A borrowed context runs the complete direct replay without a Program clone.
use super::*;
struct Borrowed<'a>(&'a ResolvedProgram);
impl crate::cleanup::ReplayContext for Borrowed<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        &self.0.declarations
    }
    fn types(&self) -> &[crate::hir::ResolvedTypeDeclaration] {
        &self.0.types
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        &self.0.interfaces
    }
    fn functions(&self) -> &[ResolvedFunction] {
        &self.0.functions
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
