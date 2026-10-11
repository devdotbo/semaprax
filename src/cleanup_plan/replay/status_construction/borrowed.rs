//! Exact guarded status-table comparison. Ordering and multiplicity stay intact.
use super::*;
pub(super) fn equal(
    function: &ResolvedFunction,
    expected: &[StatusSource],
    actual: &[StatusSource],
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "borrowed status table length")?;
    if expected.len() != actual.len() {
        return Ok(false);
    }
    for (expected, actual) in expected.iter().zip(actual) {
        budget.charge(function, 1, "borrowed status actual entry")?;
        if expected.id.lane != actual.id.lane
            || expected.id.expression.shared_allocation_key().is_some()
                != actual.id.expression.shared_allocation_key().is_some()
            || !schema::borrowed::same_schema(
                function,
                expected.id.expression.as_str(),
                actual.id.expression.as_str(),
                budget,
            )?
        {
            return Ok(false);
        }
        match (&expected.producer, &actual.producer) {
            (
                StatusProducer::PropagatedCall { callee: a },
                StatusProducer::PropagatedCall { callee: b },
            ) => {
                if !schema::borrowed::same_schema(function, a.as_str(), b.as_str(), budget)? {
                    return Ok(false);
                }
            }
            (
                StatusProducer::CheckedArithmetic {
                    operation: a,
                    normalized_cases: ac,
                },
                StatusProducer::CheckedArithmetic {
                    operation: b,
                    normalized_cases: bc,
                },
            ) => {
                budget.charge(function, 1, "borrowed arithmetic status header")?;
                if a != b || ac.len() != bc.len() {
                    return Ok(false);
                }
                for (a, b) in ac.iter().zip(bc) {
                    budget.charge(function, 1, "borrowed arithmetic actual case")?;
                    if a != b {
                        return Ok(false);
                    }
                }
            }
            (
                StatusProducer::ContractFalse {
                    phase: a,
                    ordinal: ai,
                },
                StatusProducer::ContractFalse {
                    phase: b,
                    ordinal: bi,
                },
            ) => {
                budget.charge(function, 1, "borrowed contract status header")?;
                if a != b || ai != bi {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}
