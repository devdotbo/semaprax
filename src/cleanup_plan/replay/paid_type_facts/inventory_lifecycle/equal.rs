//! Full derived ResolvedType equality without cloned trees or flattened keys.
//! Continuations retain the original left-to-right field/argument comparison.
use super::*;

enum Frame<'a> {
    Pair(&'a ResolvedType, &'a ResolvedType),
    Slice(&'a [ResolvedType], &'a [ResolvedType]),
}
fn root(
    a: &ResolvedType,
    b: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<Option<bool>, Diagnostic> {
    metadata::step(payer, 1)?;
    if std::mem::discriminant(a) != std::mem::discriminant(b) {
        return Ok(Some(false));
    }
    match (a, b) {
        (ResolvedType::ArrayU8(a), ResolvedType::ArrayU8(b)) => {
            metadata::step(payer, 1)?;
            Ok(Some(a == b))
        }
        (
            ResolvedType::TypeParameter {
                owner: a,
                index: ai,
            },
            ResolvedType::TypeParameter {
                owner: b,
                index: bi,
            },
        ) => {
            if !inventory_text::owner_matches(a, b, payer)? {
                return Ok(Some(false));
            }
            metadata::step(payer, 1)?;
            Ok(Some(ai == bi))
        }
        (
            ResolvedType::Nominal {
                declaration: a,
                arguments: aa,
            },
            ResolvedType::Nominal {
                declaration: b,
                arguments: ba,
            },
        ) => {
            if !inventory_text::owner_matches(a, b, payer)? {
                return Ok(Some(false));
            }
            metadata::step(payer, 1)?;
            if aa.len() != ba.len() {
                return Ok(Some(false));
            }
            Ok(if aa.is_empty() { Some(true) } else { None })
        }
        (
            ResolvedType::Function { parameters: a, .. },
            ResolvedType::Function { parameters: b, .. },
        ) => {
            metadata::step(payer, 1)?;
            Ok(if a.len() != b.len() {
                Some(false)
            } else {
                None
            })
        }
        _ => Ok(Some(true)),
    }
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn same(
    a: &ResolvedType,
    b: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    inventory_text::current(payer)?;
    if let Some(equal) = root(a, b, payer)? {
        return Ok(equal);
    }
    let mut pending = inventory_flags::vector(payer)?;
    schedule(a, b, &mut pending, payer)?;
    loop {
        metadata::step(payer, 1)?;
        let Some(frame) = pending.pop() else {
            return Ok(true);
        };
        match frame {
            Frame::Pair(a, b) => {
                if let Some(equal) = root(a, b, payer)? {
                    if !equal {
                        return Ok(false);
                    }
                } else {
                    schedule(a, b, &mut pending, payer)?;
                }
            }
            Frame::Slice(a, b) => {
                if let (Some((a, rest_a)), Some((b, rest_b))) = (a.split_first(), b.split_first()) {
                    inventory_flags::push(&mut pending, Frame::Slice(rest_a, rest_b), payer)?;
                    inventory_flags::push(&mut pending, Frame::Pair(a, b), payer)?;
                }
            }
        }
    }
}
fn schedule<'a>(
    a: &'a ResolvedType,
    b: &'a ResolvedType,
    pending: &mut Vec<Frame<'a>>,
    payer: &mut Payer<'_>,
) -> Result<(), Diagnostic> {
    match (a, b) {
        (
            ResolvedType::Nominal { arguments: a, .. },
            ResolvedType::Nominal { arguments: b, .. },
        ) => inventory_flags::push(pending, Frame::Slice(a, b), payer)?,
        (
            ResolvedType::Function {
                parameters: a,
                result: ar,
            },
            ResolvedType::Function {
                parameters: b,
                result: br,
            },
        ) => {
            inventory_flags::push(pending, Frame::Pair(ar, br), payer)?;
            inventory_flags::push(pending, Frame::Slice(a, b), payer)?;
        }
        _ => unreachable!("only equal compound variants schedule children"),
    }
    Ok(())
}
