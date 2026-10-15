//! Exact complementary branch and Copy-result receiving predicates.
//! The old selector/validator remains the ordinary route and independent oracle.
use super::*;
fn equal(
    function: &ResolvedFunction,
    a: &ExpressionId,
    b: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    schema::borrowed::same_schema(function, a.as_str(), b.as_str(), budget)
}
pub(super) fn validate(
    function: &ResolvedFunction,
    left: &EdgeCondition,
    right: &EdgeCondition,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    budget.charge(function, 1, "borrowed branch actual condition shapes")?;
    let valid = match (left, right) {
        (EdgeCondition::BooleanResult(a, av), EdgeCondition::BooleanResult(b, bv)) => {
            if !equal(function, a, b, budget)? {
                false
            } else {
                budget.charge(function, 1, "borrowed branch boolean polarity")?;
                av != bv
            }
        }
        (
            EdgeCondition::VariantCase {
                scrutinee: a,
                case: ac,
                matches: am,
            },
            EdgeCondition::VariantCase {
                scrutinee: b,
                case: bc,
                matches: bm,
            },
        ) => {
            if !equal(function, a, b, budget)?
                || !schema::borrowed::same_schema(function, ac.as_str(), bc.as_str(), budget)?
            {
                false
            } else {
                budget.charge(function, 1, "borrowed branch variant polarity")?;
                am != bm
            }
        }
        (
            EdgeCondition::ArmSelected {
                scrutinee: a,
                arm: aa,
                selected: av,
            },
            EdgeCondition::ArmSelected {
                scrutinee: b,
                arm: ba,
                selected: bv,
            },
        ) => {
            if !equal(function, a, b, budget)? {
                false
            } else {
                budget.charge(function, 1, "borrowed branch exact arm ordinal")?;
                if aa != ba {
                    false
                } else {
                    budget.charge(function, 1, "borrowed branch arm polarity")?;
                    av != bv
                }
            }
        }
        (EdgeCondition::StatusZero(a), EdgeCondition::StatusNonzero(b))
        | (EdgeCondition::StatusNonzero(a), EdgeCondition::StatusZero(b)) => {
            if !equal(function, &a.expression, &b.expression, budget)? {
                false
            } else {
                budget.charge(function, 1, "borrowed branch exact status lane")?;
                a.lane == b.lane
            }
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(replay_error(
            function,
            "cleanup branch conditions are not complementary",
        ))
    }
}
pub(super) fn staged(
    function: &ResolvedFunction,
    source: &StagedCopyResultSource,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let StagedCopyResultSource::Body {
        expression,
        instance,
    } = source
    else {
        return Err(replay_error(
            function,
            "selected structural replay does not support residual staging",
        ));
    };
    if !equal(function, expression, &function.body.id, budget)? {
        return Err(replay_error(
            function,
            "body result staging references another expression",
        ));
    }
    // The bound profile's declared result is scalar: comparison never descends
    // into a variable nominal/layout payload from a hostile attached instance.
    budget.charge(function, 1, "borrowed Copy stage actual result type header")?;
    if instance != &function.return_type {
        return Err(replay_error(
            function,
            "staged Copy result targets another concrete function result",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests;
