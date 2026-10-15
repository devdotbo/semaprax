//! Selective lexical source replay; frozen recipe-v8 admission remains unchanged.
use super::{package_error, semantic_recipe_v8 as legacy};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

const HEADER: &str = "// semaprax.owned-data-semantic-recipe.lexical.v1\n";
const MAX_BYTES: usize = 1024 * 1024;

fn selected(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    for function in &program.functions {
        if crate::hir::lexical_exit::requires(function)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn render(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    if !selected(program)? {
        return legacy::render(program);
    }
    // The marker is selection, never authority. Replay authenticates every
    // target, operand route, latch, loan and cleanup certificate independently.
    crate::hir::validate(program)?;
    let body = legacy::render_profile(program, true)?;
    if body
        .len()
        .checked_add(HEADER.len())
        .is_none_or(|n| n > MAX_BYTES)
    {
        return Err(package_error(
            "lexical semantic recipe exceeds its byte bound",
        ));
    }
    Ok(format!("{HEADER}{body}"))
}

pub(super) fn replay(recipe: &str) -> Result<ResolvedProgram, Diagnostic> {
    let Some(body) = recipe.strip_prefix(HEADER) else {
        return legacy::replay(recipe);
    };
    if recipe.len() > MAX_BYTES {
        return Err(package_error(
            "lexical semantic recipe exceeds its byte bound",
        ));
    }
    let program = legacy::replay_profile(body, true)?;
    if !selected(&program)? || render(&program)? != recipe {
        return Err(package_error(
            "lexical semantic recipe selection is not canonical",
        ));
    }
    Ok(program)
}

pub(super) fn replay_against(
    linked: &ResolvedProgram,
    recipe: &str,
) -> Result<ResolvedProgram, Diagnostic> {
    if !selected(linked)? {
        return legacy::replay_against(linked, recipe);
    }
    if render(linked)? != recipe {
        return Err(package_error(
            "lexical semantic recipe disagrees with linked HIR",
        ));
    }
    let replayed = replay(recipe)?;
    if linked.entrypoint != replayed.entrypoint
        || legacy::declaration_inventory(linked) != legacy::declaration_inventory(&replayed)
        || legacy::function_inventory(linked) != legacy::function_inventory(&replayed)
    {
        return Err(package_error(
            "lexical semantic recipe changes the linked inventory",
        ));
    }
    Ok(replayed)
}

#[cfg(test)]
mod tests;
