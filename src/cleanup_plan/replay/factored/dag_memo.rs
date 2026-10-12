//! Exact DAG identity/continuation memo receiving; no pointer-only sharing.
//! Guarded keys use session construction order, never allocator address order.
//! Every cached ordinal is still authenticated against the exact current Rc.
use super::*;
#[cfg(test)]
pub(super) mod ordinal;
#[cfg(not(test))]
pub(super) type Memo = BTreeMap<(usize, usize), (usize, usize)>;
#[cfg(test)]
pub(super) enum Memo {
    Ordinary(BTreeMap<(usize, usize), (usize, usize)>),
    Borrowed(ordinal::Ordinals),
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
        return Ok(Memo::Borrowed(ordinal::Ordinals::new()));
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
#[cfg(test)]
pub(super) fn test_borrowed() -> Memo {
    Memo::Borrowed(ordinal::Ordinals::new())
}
pub(super) struct Key {
    pointer: usize,
    continuation: usize,
    ordinal: Option<usize>,
}
pub(super) fn key(dag: &Rc<ObservationDag>, continuation: usize) -> Key {
    Key {
        pointer: Rc::as_ptr(dag) as usize,
        continuation,
        ordinal: dag.ordinal,
    }
}
#[cfg(test)]
pub(super) fn test_key(pointer: usize, ordinal: usize, continuation: usize) -> Key {
    Key {
        pointer,
        ordinal: Some(ordinal),
        continuation,
    }
}
pub(super) fn get(
    function: &ResolvedFunction,
    memo: &Memo,
    key: &Key,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    #[cfg(test)]
    {
        match memo {
            Memo::Ordinary(memo) => ordinary_get(function, memo, key, budget),
            Memo::Borrowed(memo) => {
                let ordinal = key.ordinal.ok_or_else(|| {
                    replay_error(
                        function,
                        "borrowed DAG memo has no held construction ordinal",
                    )
                })?;
                let Some((pointer, end)) =
                    memo.get(function, &(ordinal, key.continuation), budget)?
                else {
                    return Ok(None);
                };
                budget.charge(function, 1, "borrowed DAG memo exact current pointer")?;
                if *pointer != key.pointer {
                    return Err(replay_error(
                        function,
                        "borrowed DAG memo construction identity collides",
                    ));
                }
                Ok(Some(*end))
            }
        }
    }
    #[cfg(not(test))]
    {
        ordinary_get(function, memo, key, budget)
    }
}
pub(super) fn insert(
    function: &ResolvedFunction,
    memo: &mut Memo,
    key: Key,
    end: usize,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let ordinary = match memo {
        Memo::Borrowed(memo) => {
            let ordinal = key.ordinal.ok_or_else(|| {
                replay_error(
                    function,
                    "borrowed DAG memo has no held construction ordinal",
                )
            })?;
            let numeric = (ordinal, key.continuation);
            if let Some((pointer, _)) = memo.get(function, &numeric, budget)? {
                budget.charge(function, 1, "borrowed DAG memo exact insertion pointer")?;
                if *pointer != key.pointer {
                    return Err(replay_error(
                        function,
                        "borrowed DAG memo construction identity collides",
                    ));
                }
            }
            return memo.insert(function, numeric, (key.pointer, end), budget);
        }
        Memo::Ordinary(memo) => memo,
    };
    #[cfg(not(test))]
    let ordinary = memo;
    let ordinal = key.ordinal.ok_or_else(|| {
        replay_error(function, "ordinary DAG memo has no construction ordinal")
    })?;
    let numeric = (ordinal, key.continuation);
    budget.charge(function, 1, "ordinary DAG memo exact insertion pointer")?;
    if let Some((pointer, _)) = ordinary.get(&numeric) {
        if *pointer != key.pointer {
            return Err(replay_error(function, "ordinary DAG memo construction identity collides"));
        }
    } else {
        budget.reserve_btree_node::<(usize, usize), (usize, usize)>(
            function, "factored DAG identity memo",
        )?;
    }
    #[cfg(test)]
    instrumentation::memo(numeric, ordinary);
    ordinary.insert(numeric, (key.pointer, end));
    Ok(())
}

fn ordinary_get(
    function: &ResolvedFunction,
    memo: &BTreeMap<(usize, usize), (usize, usize)>,
    key: &Key,
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    let ordinal = key.ordinal.ok_or_else(|| {
        replay_error(function, "ordinary DAG memo has no construction ordinal")
    })?;
    let Some((pointer, end)) = memo.get(&(ordinal, key.continuation)) else {
        return Ok(None);
    };
    budget.charge(function, 1, "ordinary DAG memo exact current pointer")?;
    if *pointer != key.pointer {
        return Err(replay_error(function, "ordinary DAG memo construction identity collides"));
    }
    Ok(Some(*end))
}

#[cfg(test)]
mod tests;
