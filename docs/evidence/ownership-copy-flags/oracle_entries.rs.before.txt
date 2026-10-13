//! Storage routing around complete original receivers; their bodies stay saved.
use super::*;
impl HirValidator<'_> {
    pub(in crate::hir::validation) fn use_legacy_identities(&mut self) {
        self.expression_ids.use_legacy();
        self.value_ids.use_legacy();
    }
    pub(in crate::hir::validation) fn validate_function_owned_reference(
        &mut self,
        function: &ResolvedFunction,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        self.use_legacy_identities();
        self.validate_function_owned_reference_body(function, execution)
    }
    pub(in crate::hir) fn validate_expr_iterative_owned_reference(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<effect_keys::Allowed<'_>>,
    ) -> Result<(), Diagnostic> {
        self.use_legacy_identities();
        self.validate_expr_iterative_owned_reference_body(
            function,
            expression,
            scope,
            path,
            allow_moves,
            allowed_effects,
        )
    }
    pub(in crate::hir) fn validate_expr_recursive_reference(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        self.use_legacy_identities();
        self.validate_expr_recursive_reference_body(
            function,
            expression,
            scope,
            path,
            allow_moves,
            allowed_effects,
        )
    }
    pub(in crate::hir::validation) fn validate_closure_owned_reference(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
        path: &str,
        allow_moves: bool,
    ) -> Result<(), Diagnostic> {
        self.use_legacy_identities();
        self.validate_closure_owned_reference_body(function, expression, scope, path, allow_moves)
    }
    pub(in crate::hir::validation) fn validate_template_closure_owned_reference<'value>(
        &mut self,
        template: &'value ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &'value ResolvedExpr,
        values: &mut generic_template::values::Values<'value>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        self.use_legacy_identities();
        self.validate_template_closure_owned_reference_body(
            template, execution, expression, values, path,
        )
    }
}
