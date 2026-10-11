//! Guarded complete borrowed helper replay; production admission stays closed.
use super::*;
impl Preparation {
    fn append_option_authority(
        &mut self,
        function: &ResolvedFunction,
        declarations: &mut crate::hir::DeclarationIndex,
    ) -> Result<(), Diagnostic> {
        self.construct(function, |physical, debit| {
            declarations.append_option_u8_replay_metered(physical, debit)
        })
    }
}
impl Session<'_> {
    fn borrowed_replay(&mut self, function: &ResolvedFunction) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected module replay already failed",
            ));
        }
        self.failed = true;
        let context =
            selected_context::borrowed::Context::new(&self.indexed, function, &mut self.budget)?;
        scalar_replay::validate(&context, function, &mut self.budget)?;
        self.failed = false;
        Ok(())
    }
    fn borrowed_receivers(&mut self, function: &ResolvedFunction) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(replay_error(
                function,
                "selected module replay already failed",
            ));
        }
        self.failed = true;
        let context =
            selected_context::borrowed::Context::new(&self.indexed, function, &mut self.budget)?;
        let sites = context.primitive_renewals(function).ok_or_else(|| {
            replay_error(
                function,
                "guarded borrowed renewal lease lost its actual member",
            )
        })?;
        if sites.has_helpers() {
            return Err(replay_error(
                function,
                "guarded borrowed renewal identity disagrees",
            ));
        }
        drop(context);
        self.failed = false;
        Ok(())
    }
}
#[cfg(test)]
mod tests;
