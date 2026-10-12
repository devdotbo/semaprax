//! Exact replay of immutable facts constructed from held authoritative source.
//! A schema token or stored selection bit is never literal-root authority.
use super::*;
use std::fmt::Write;
mod primitive;
pub(super) use primitive::replay_paid as replay_lexical_paid;
pub(super) use primitive::current_module;

const WORK: usize = 1_048_576;

/// The public retained-source literal, also the legacy pass-through form a
/// paid selected replay carrier publishes without allocation.
pub(in crate::workspace_graph) const REPORT: crate::cleanup_plan::SelectedReplayReportForm =
    crate::cleanup_plan::SelectedReplayReportForm::new(
        "SPX-G410",
        "retained static UTF-8 source-site authority disagrees",
    );

fn fail() -> Vec<Diagnostic> {
    REPORT.legacy()
}
struct Compare<'a>(&'a str);
impl Write for Compare<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.0 = self.0.strip_prefix(text).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
fn monomorphic_site(site: &crate::workspace_graph::static_text::Site) -> bool {
    let mut output = Compare(site.expression.as_str());
    write!(
        output,
        "declaration:{}:{}:expression:{}:{}",
        site.owner.as_str().len(),
        site.owner,
        site.path.len(),
        site.path
    )
    .is_ok()
        && output.0.is_empty()
}

fn instance_site(
    expression: &hir::ExpressionId,
    instance: &hir::FunctionInstanceId,
    path: &str,
) -> bool {
    const PREFIX: &str = "semaprax.function-execution.v1:generic:";
    let digits = |value: usize| {
        if value == 0 {
            1
        } else {
            value.ilog10() as usize + 1
        }
    };
    let Some(owner_length) = PREFIX
        .len()
        .checked_add(digits(instance.as_str().len()))
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_add(instance.as_str().len()))
    else {
        return false;
    };
    let mut output = Compare(expression.as_str());
    write!(
        output,
        "function-execution:{owner_length}:{PREFIX}{}:{}:expression:{}:{path}",
        instance.as_str().len(),
        instance.as_str(),
        path.len()
    )
    .is_ok()
        && output.0.is_empty()
}

pub(super) fn schema(
    module: &WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<Option<&'static str>, Vec<Diagnostic>> {
    let Some(inventory) = &module.static_text else {
        return Ok(None);
    };
    if inventory.path != module.path
        || inventory.module != module.module
        || inventory.sites.is_empty()
    {
        return Err(fail());
    }
    let mut work = WORK;
    let mut matching_work = WORK;
    for (index, site) in inventory.sites.iter().enumerate() {
        matching_work = matching_work.checked_sub(index).ok_or_else(fail)?;
        let origin = facts.get(site.owner.as_str()).ok_or_else(fail)?;
        if origin.kind != hir::DeclarationKind::Function
            || origin.owner.is_some()
            || origin.origin
                != if site.explicit {
                    hir::IdentityOrigin::Explicit
                } else {
                    hir::IdentityOrigin::Automatic
                }
            || origin.path.as_deref() != Some(module.path.as_str())
            || origin.module.as_deref() != Some(module.module.as_str())
            || !site.text.is_backed()
            || !monomorphic_site(site)
            || inventory.sites[..index].iter().any(|previous| {
                previous.expression == site.expression
                    || previous.owner == site.owner && previous.span == site.span
            })
        {
            return Err(fail());
        }
    }
    let count = inventory.sites.len();
    reserve_builder_structure(
        count
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or_else(fail)?,
    )?;
    let mut seen = vec![0usize; count];
    if seen.capacity() > count {
        reserve_builder_structure(
            (seen.capacity() - count)
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(fail)?,
        )?;
    }
    let mut decoded = 0usize;
    for (owner, roots) in module
        .functions
        .iter()
        .map(|f| {
            (
                &f.id,
                f.requires
                    .iter()
                    .chain(std::iter::once(&f.body))
                    .chain(&f.ensures),
            )
        })
        .chain(module.function_templates.iter().map(|f| {
            (
                &f.id,
                f.requires
                    .iter()
                    .chain(std::iter::once(&f.body))
                    .chain(&f.ensures),
            )
        }))
    {
        hir::static_text::visit_roots_with_work(roots, &mut work, |expression, text, literal| {
            decoded = decoded
                .checked_add(text.len())
                .ok_or_else(|| graph_error("SPX-G410", "retained decoded pool overflows"))?;
            if !literal {
                return Ok(());
            }
            if expression.ty != hir::ResolvedType::Str
                || expression.ownership != hir::OwnershipMode::Borrow
            {
                return Err(graph_error(
                    "SPX-G410",
                    "static source producer type or ownership disagrees",
                ));
            }
            matching_work = matching_work
                .checked_sub(count)
                .ok_or_else(|| graph_error("SPX-G410", "static site replay exceeds work limit"))?;
            let mut matches = inventory.sites.iter().enumerate().filter(|(_, site)| {
                site.owner == *owner
                    && site.expression == expression.id
                    && site.span == expression.span
                    && site.text.as_str() == text
            });
            let (index, _) = matches.next().ok_or_else(|| {
                graph_error("SPX-G410", "static retained producer is unauthenticated")
            })?;
            if matches.next().is_some() {
                return Err(graph_error("SPX-G410", "duplicate static producer"));
            }
            seen[index] = seen[index]
                .checked_add(1)
                .ok_or_else(|| graph_error("SPX-G410", "static site count overflows"))?;
            Ok(())
        })
        .map_err(|e| vec![e])?;
    }
    if seen.iter().any(|count| *count != 1) {
        return Err(fail());
    }
    for instance in &module.function_instances {
        // The same precharged carrier authenticates the exact template-site
        // bijection for every instance. Charge reset, expected-site census and
        // final bijection comparison before any of those walks.
        matching_work = matching_work
            .checked_sub(count.checked_mul(3).ok_or_else(fail)?)
            .ok_or_else(fail)?;
        seen.fill(0);
        if instance.function.id != instance.template {
            return Err(fail());
        }
        let expected = inventory
            .sites
            .iter()
            .filter(|site| site.owner == instance.template)
            .count();
        let mut observed = 0usize;
        hir::static_text::visit_roots_with_work(
            instance
                .function
                .requires
                .iter()
                .chain(std::iter::once(&instance.function.body))
                .chain(&instance.function.ensures),
            &mut work,
            |expression, text, literal| {
                decoded = decoded.checked_add(text.len()).ok_or_else(|| {
                    graph_error("SPX-G410", "retained instance decoded pool overflows")
                })?;
                if !literal {
                    return Ok(());
                }
                if expression.ty != hir::ResolvedType::Str
                    || expression.ownership != hir::OwnershipMode::Borrow
                {
                    return Err(graph_error(
                        "SPX-G410",
                        "static instance producer type or ownership disagrees",
                    ));
                }
                matching_work = matching_work.checked_sub(count).ok_or_else(|| {
                    graph_error("SPX-G410", "static instance replay exceeds work limit")
                })?;
                let mut matches = inventory.sites.iter().enumerate().filter(|(_, site)| {
                    site.owner == instance.template
                        && site.span == expression.span
                        && site.text.as_str() == text
                        && instance_site(&expression.id, &instance.id, &site.path)
                });
                let Some((index, _)) = matches.next() else {
                    return Err(graph_error(
                        "SPX-G410",
                        "static generic producer lacks authenticated template site",
                    ));
                };
                if matches.next().is_some() {
                    return Err(graph_error(
                        "SPX-G410",
                        "static generic producer lacks exact authenticated template site",
                    ));
                }
                seen[index] = seen[index].checked_add(1).ok_or_else(|| {
                    graph_error("SPX-G410", "static instance site count overflows")
                })?;
                observed = observed
                    .checked_add(1)
                    .ok_or_else(|| graph_error("SPX-G410", "static instance count overflows"))?;
                Ok(())
            },
        )
        .map_err(|e| vec![e])?;
        if expected != observed
            || inventory
                .sites
                .iter()
                .zip(&seen)
                .any(|(site, seen)| *seen != usize::from(site.owner == instance.template))
        {
            return Err(fail());
        }
    }
    if decoded > hir::static_text::MAX_POOL_BYTES {
        return Err(fail());
    }
    Ok(Some("semaprax.graph.v77"))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::workspace_graph) use primitive::{with_prepared_test_context, with_test_context};
