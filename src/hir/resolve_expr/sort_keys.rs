//! The literal descriptor is checked metadata, never a String expression.
use super::path_arena::{ExprPath, PathArena};
use super::*;
impl Resolver<'_> {
    pub(super) fn finish_vec_literal_frame(
        &self,
        function: &FunctionExecutionId,
        frame: Frame<'_>,
        paths: &PathArena<'_>,
        results: &mut Vec<ResolvedExpr>,
    ) -> Result<(), Diagnostic> {
        match frame {
            Frame::FinishSortBy {
                span,
                path,
                element,
                callee,
            } => {
                let args = take_results(results, 1);
                let path = paths.at(path);
                results.push(self.finish_sort_by(function, span, path, element, callee, args)?);
            }
            Frame::FinishVecField {
                span,
                path,
                element,
                field,
            } => {
                let args = take_results(results, 2);
                let path = paths.at(path);
                results.push(self.finish_vec_field(function, span, path, element, field, args)?);
            }
            _ => unreachable!("literal vector frame"),
        }
        Ok(())
    }
    pub(in crate::hir) fn prepare_sort_by(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        types: &[crate::ast::Type],
        args: &[Expr],
    ) -> Result<(ResolvedType, DeclarationId), Diagnostic> {
        let fail = || {
            self.error("SPX-H006","vec_sort_by requires a monomorphic explicit record and one finite literal key list",expression.span)
        };
        if function.instance().is_some() {
            return Err(fail());
        }
        let [ty] = types else {
            return Err(fail());
        };
        let [_, literal] = args else {
            return Err(fail());
        };
        let ExprKind::String(keys) = &literal.kind else {
            return Err(fail());
        };
        let element = self.resolve_type(ty, expression.span)?;
        let descriptor = crate::vec_ops::sort_keys::resolve(&self.declarations, &element, keys)
            .and_then(|d| d.identity())
            .ok_or_else(fail)?;
        Ok((element, DeclarationId::new(descriptor)))
    }
    pub(in crate::hir) fn finish_sort_by<'p>(
        &self,
        function: &FunctionExecutionId,
        span: crate::ast::Span,
        path: impl Into<ExprPath<'p>>,
        element: ResolvedType,
        callee: DeclarationId,
        args: Vec<ResolvedExpr>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let mut expression = super::super::resolve_vec_call::finish(
            self,
            function,
            path,
            span,
            crate::vec_ops::VecOp::SortBy,
            element,
            args,
        )?;
        let ResolvedExprKind::Call { callee: target, .. } = &mut expression.kind else {
            unreachable!("vector call")
        };
        *target = callee;
        Ok(expression)
    }
    pub(super) fn sort_by_frames<'e>(
        &self,
        function: &FunctionExecutionId,
        expression: &Expr,
        types: &[crate::ast::Type],
        args: &'e [Expr],
        bindings: Rc<BTreeMap<String, Binding>>,
        path: PathId,
    ) -> Result<[Frame<'e>; 2], Diagnostic> {
        let (element, callee) = self.prepare_sort_by(function, expression, types, args)?;
        Ok([
            Frame::FinishSortBy {
                span: expression.span,
                path,
                element,
                callee,
            },
            Frame::ChildNext {
                children: &args[..1],
                index: 0,
                bindings,
                path,
                segment: Seg::Arg,
            },
        ])
    }
}
