//! Complete the selected owning wrapper without entering its ordinary exit.
use super::*;
use crate::ast::LexicalExitKind;
use crate::hir::ControlTarget;

impl<O: COutput> CEmitter<'_, O> {
    fn wrapper_break_target(
        &self,
        wrapper: &ResolvedExpr,
    ) -> Result<Option<&ControlTarget>, Diagnostic> {
        let ResolvedExprKind::Block { statements, .. } = &wrapper.kind else {
            return Ok(None);
        };
        let Some(ResolvedStatement::LexicalWhile {
            condition,
            latch: hir::LoopLatchKind::OwnedIteratorStep { .. },
            ..
        }) = statements.last()
        else {
            return Ok(None);
        };
        let Some(control) = self.function.cleanup_plan.lexical_control.as_deref() else {
            return Ok(None);
        };
        let Some(certificate) = self.function.cleanup_plan.lexical_loop(&condition.id) else {
            return Err(backend_error(
                "native owning wrapper has no exact loop certificate",
            ));
        };
        if control.exits.iter().any(|exit| {
            exit.kind == LexicalExitKind::Break
                && exit.target == certificate.target
                && exit.resume.is_some()
        }) {
            hir::lexical_exit::source_for_wrapper(self.program, wrapper)?
                .ok_or_else(|| backend_error("native owning wrapper lost source authority"))?;
            Ok(Some(&certificate.target))
        } else {
            Ok(None)
        }
    }

    pub(super) fn emit_owned_break_join_label(
        &mut self,
        wrapper: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        if let Some(target) = self.wrapper_break_target(wrapper)? {
            let label = format!(
                "spx_lexical_wrapper_after_{}",
                self.lexical_loop_index(target)?
            );
            self.label(&label);
        }
        Ok(())
    }

    pub(super) fn emit_owned_wrapper_break(
        &mut self,
        target: &ControlTarget,
        entry: crate::cleanup_plan::BlockId,
    ) -> Result<(), Diagnostic> {
        let ControlTarget::Loop { site, .. } = target else {
            return Err(backend_error("native owning Break target is not a loop"));
        };
        let wrapper =
            super::lexical_lookup::owning_wrapper(self.program, &self.function.body, site)?;
        if self.wrapper_break_target(wrapper)? != Some(target) {
            return Err(backend_error(
                "native owning Break resume lacks exact wrapper route",
            ));
        }
        let ResolvedExprKind::Block { tail, .. } = &wrapper.kind else {
            unreachable!()
        };
        self.in_cleanup_resume(entry, |emitter| {
            let value = emitter.emit_expr(tail)?;
            emitter.require_type(&value.ty, &wrapper.ty, "owning Break wrapper tail")?;
            emitter.line(&format!("(void)({});", value.code));
            emitter.emit_block_plan_scope_exit(wrapper)
        })?;
        let label = format!(
            "spx_lexical_wrapper_after_{}",
            self.lexical_loop_index(target)?
        );
        self.line(&format!("goto {label};"));
        Ok(())
    }
}
