//! One bounded caller-held directory and an exact paid large-group fallback.
use super::{refused, CfgReservation, Diagnostic, Group, LoanPlan, ResolvedProgram};
pub(super) const INLINE_PLANS: usize = 32;
pub(super) struct Staging {
    inline: [Option<LoanPlan>; INLINE_PLANS],
    heap: Vec<LoanPlan>,
    len: usize,
    large: bool,
}
impl Staging {
    pub(super) fn new() -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            heap: Vec::new(),
            len: 0,
            large: false,
        }
    }
    pub(super) fn push(
        &mut self,
        plan: LoanPlan,
        count: usize,
        owner: &CfgReservation,
    ) -> Result<(), Diagnostic> {
        if self.len == 0 {
            self.large = count > INLINE_PLANS;
            if self.large {
                owner.array::<LoanPlan>(count).map_err(|_| refused())?;
                let requested = std::alloc::Layout::array::<LoanPlan>(count)
                    .expect("array layout checked by exact owner")
                    .size();
                if self.heap.try_reserve_exact(count).is_err() {
                    owner.allocation(requested);
                    return Err(refused());
                }
                if self.heap.capacity() != count {
                    let excess = self
                        .heap
                        .capacity()
                        .saturating_sub(count)
                        .saturating_mul(std::mem::size_of::<LoanPlan>());
                    let _ = owner.capacity(requested, excess);
                    return Err(refused());
                }
            }
        }
        owner.reserve(0).map_err(|_| refused())?;
        if self.large {
            self.heap.push(plan);
        } else {
            self.inline[self.len] = Some(plan);
        }
        self.len += 1;
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn empty_unallocated(&self) -> bool {
        self.len == 0
            && self.heap.is_empty()
            && self.heap.capacity() == 0
            && self.inline.iter().all(Option::is_none)
    }
    pub(super) fn publish(&mut self, program: &mut ResolvedProgram, group: Group) {
        debug_assert_eq!(self.len, group.len(program));
        if self.large {
            match group {
                Group::Authored => {
                    for (function, plan) in program.functions.iter_mut().zip(self.heap.drain(..)) {
                        function.loan_plan = plan;
                    }
                }
                Group::Instances => {
                    for (instance, plan) in program
                        .function_instances
                        .iter_mut()
                        .zip(self.heap.drain(..))
                    {
                        instance.function.loan_plan = plan;
                    }
                }
            }
            // Match the original directory's disposal before the next planner.
            drop(std::mem::take(&mut self.heap));
        } else {
            for index in 0..self.len {
                let plan = self.inline[index]
                    .take()
                    .expect("complete group before publication");
                match group {
                    Group::Authored => program.functions[index].loan_plan = plan,
                    Group::Instances => program.function_instances[index].function.loan_plan = plan,
                }
            }
        }
        self.len = 0;
        self.large = false;
    }
}
