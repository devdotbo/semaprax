//! Exact DAG identity/continuation memo receiving; no pointer-only sharing.
use super::*;
#[cfg(not(test))]
pub(super) type Memo = BTreeMap<(usize, usize), usize>;
#[cfg(test)]
pub(super) enum Memo {
    Ordinary(BTreeMap<(usize, usize), usize>),
    Borrowed(borrowed_canonical::dictionary::Dictionary<(usize, usize), usize>),
}
pub(super) fn new(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Memo, Diagnostic> {
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        lease.validate(program, function, budget)?;
        budget.reserve_array::<Memo>(function, 1, "borrowed exact DAG memo header")?;
        return Ok(Memo::Borrowed(
            borrowed_canonical::dictionary::Dictionary::new(),
        ));
    }
    #[cfg(not(test))]
    let _ = (program, function, budget);
    #[cfg(test)]
    {
        Ok(Memo::Ordinary(BTreeMap::new()))
    }
    #[cfg(not(test))]
    {
        Ok(BTreeMap::new())
    }
}
pub(super) fn get(
    function: &ResolvedFunction,
    memo: &Memo,
    key: &(usize, usize),
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    #[cfg(test)]
    {
        match memo {
            Memo::Ordinary(memo) => Ok(memo.get(key).copied()),
            Memo::Borrowed(memo) => Ok(memo.get(function, key, budget)?.copied()),
        }
    }
    #[cfg(not(test))]
    {
        let _ = (function, budget);
        Ok(memo.get(key).copied())
    }
}
pub(super) fn insert(
    function: &ResolvedFunction,
    memo: &mut Memo,
    key: (usize, usize),
    end: usize,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let ordinary = match memo {
        Memo::Borrowed(memo) => return memo.insert(function, key, end, budget),
        Memo::Ordinary(memo) => memo,
    };
    #[cfg(not(test))]
    let ordinary = memo;
    if !ordinary.contains_key(&key) {
        budget
            .reserve_btree_node::<(usize, usize), usize>(function, "factored DAG identity memo")?;
    }
    #[cfg(test)]
    instrumentation::memo(key, ordinary);
    ordinary.insert(key, end);
    Ok(())
}
