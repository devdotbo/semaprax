//! Independent scalar-helper fact construction and actual ordered key probes.
//! This table never acquires a separate work allowance or a legacy tree index.
use super::*;
use std::cmp::Ordering;

pub(in crate::cleanup_plan::replay) struct BorrowedFacts<'hir> {
    entries: Vec<(ExpressionId, Option<CallFact<'hir>>)>,
}
fn compare(
    function: &ResolvedFunction,
    a: &ExpressionId,
    b: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    // ExpressionId's original Ord compares its text, including empty carriers.
    for (a, b) in a.as_str().bytes().zip(b.as_str().bytes()) {
        budget.charge(function, 1, "borrowed fact actual key byte")?;
        match a.cmp(&b) {
            Ordering::Equal => {}
            order => return Ok(order),
        }
    }
    budget.charge(function, 1, "borrowed fact actual key length")?;
    Ok(a.as_str().len().cmp(&b.as_str().len()))
}
impl<'hir> BorrowedFacts<'hir> {
    fn position(
        &self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<Result<usize, usize>, Diagnostic> {
        let mut low = 0;
        let mut high = self.entries.len();
        while low < high {
            budget.charge(function, 1, "borrowed fact actual table probe")?;
            let middle = low + (high - low) / 2;
            match compare(function, &self.entries[middle].0, key, budget)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(Ok(middle)),
            }
        }
        Ok(Err(low))
    }
    pub(in crate::cleanup_plan::replay) fn build(
        function: &'hir ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        budget.reserve_array::<Self>(function, 1, "borrowed fact table header")?;
        budget.reserve_array::<FactFrame<'_>>(function, 514, "borrowed fact cursor")?;
        let mut table = Self {
            entries: Vec::new(),
        };
        let mut stack = [None; 514];
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            stack[0] = Some((expression_children::ReplayChildCursor::new(root), false));
            let mut depth = 1;
            while depth != 0 {
                budget.charge(function, 1, "borrowed fact actual cursor frame")?;
                depth -= 1;
                let (mut cursor, started) =
                    stack[depth].take().expect("retained borrowed fact frame");
                let expression = cursor.expression();
                if !started {
                    let position = match table.position(function, &expression.id, budget)? {
                        Err(position) => position,
                        Ok(_) => {
                            return Err(replay_error(
                                function,
                                format_args!(
                                    "HIR expression identity `{}` is repeated",
                                    expression.id
                                ),
                            ))
                        }
                    };
                    budget.charge(function, 1, "borrowed fact actual expression kind")?;
                    let fact = match &expression.kind {
                        ResolvedExprKind::Call {
                            callee,
                            instance,
                            args,
                            type_arguments,
                        } => {
                            // The complete profile independently checked this closed
                            // monomorphic shape; reconstruct the fact from actual HIR.
                            if instance.is_some() || !type_arguments.is_empty() {
                                return Err(replay_error(
                                    function,
                                    "borrowed expression fact is not monomorphic",
                                ));
                            }
                            budget.charge(
                                function,
                                callee.as_str().len(),
                                "borrowed fact callee copy bytes",
                            )?;
                            budget.reserve_clone(function, callee, "borrowed fact callee clone")?;
                            budget.reserve_array::<ExpressionId>(
                                function,
                                args.len(),
                                "borrowed fact argument slots",
                            )?;

                            for _argument in args {
                                budget.charge(
                                    function,
                                    1,
                                    "borrowed fact actual argument identity clone",
                                )?;
                            }
                            Some(CallFact {
                                callee: std::borrow::Cow::Borrowed(callee),
                                instance: None,
                                arguments: call_payload::Arguments::Slice(args),
                                type_arguments: call_payload::Types::Slice(&[]),
                            })
                        }
                        ResolvedExprKind::Invoke { .. }
                        | ResolvedExprKind::Closure { .. }
                        | ResolvedExprKind::FunctionReference { .. }
                        | ResolvedExprKind::ByteRange { .. }
                        | ResolvedExprKind::HostCommandCall(_)
                        | ResolvedExprKind::NativeRustImportCall(_)
                        | ResolvedExprKind::LiteralFormat { .. }
                        | ResolvedExprKind::VecFieldRead { .. } => {
                            return Err(replay_error(function, "borrowed expression facts contain an unsupported delegated operation"));
                        }
                        _ => None,
                    };
                    if table.entries.len() == table.entries.capacity() {
                        budget.charge(
                            function,
                            table.entries.len(),
                            "borrowed fact actual backing relocation",
                        )?;
                    }
                    budget.reserve_vec(
                        function,
                        &mut table.entries,
                        1,
                        "borrowed ordered expression facts",
                    )?;
                    budget.charge(
                        function,
                        table.entries.len() - position + 1,
                        "borrowed fact actual entry shifts and identity clone",
                    )?;
                    table
                        .entries
                        .insert(position, (expression.id.clone(), fact));
                }
                if let Some(child) = cursor.next() {
                    if depth + 2 > stack.len() {
                        return Err(replay_error(
                            function,
                            "HIR expression fact traversal exceeds the admitted depth",
                        ));
                    }
                    stack[depth] = Some((cursor, true));
                    stack[depth + 1] =
                        Some((expression_children::ReplayChildCursor::new(child), false));
                    depth += 2;
                }
            }
        }
        Ok(table)
    }
}
impl<'hir> Lookup for BorrowedFacts<'hir> {
    fn is_metered(&self) -> bool {
        true
    }
    fn entries(&self) -> Entries<'_> {
        Entries::Borrowed(self.entries.iter())
    }
    fn fact<'a>(
        &'a self,
        function: &ResolvedFunction,
        key: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<Option<&'a Option<CallFact<'a>>>, Diagnostic> {
        Ok(match self.position(function, key, budget)? {
            Ok(position) => Some(&self.entries[position].1),
            Err(_) => None,
        })
    }
}

#[cfg(test)]
mod tests;
