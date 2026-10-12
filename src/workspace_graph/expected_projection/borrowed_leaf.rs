//! Exact-source borrowed input for the generated scalar-main leaf route.
//! Imported/private layouts and invariant desugaring retain the owned route.
use super::{
    active_builder_limit, checked_usage, graph_error, limit_error, private_layout,
    reserve_builder_structure, synthetic_main_runtime_cost, synthetic_program_with_registry,
    AuthoredDeclaration,
};
use crate::ast::program_source::SyntheticProgramView;
use crate::ast::Span;
use crate::ast::{Expr, ExprKind, Function, Program, Type, TypeDeclaration};
use crate::bounded_output::budgeted_format;
use crate::diagnostic::Diagnostic;
use crate::hir;
use std::collections::BTreeMap;

pub(in crate::workspace_graph) fn eligible(program: &Program) -> bool {
    program.module_uses.is_empty()
        && program
            .types
            .iter()
            .all(|declaration| declaration.invariants().is_empty())
}

pub(in crate::workspace_graph) fn construct(
    program: &Program,
) -> Result<SyntheticProgramView<'_>, Vec<Diagnostic>> {
    if !eligible(program) {
        return Err(vec![graph_error(
            "SPX-G173",
            "borrowed projection requires the owned route",
        )]);
    }
    // Preserve the owned producer's type-name preflight and its diagnostic
    // order. This directory borrows actual authored declarations; no body or
    // declaration is materialized merely to run the ordinary check.
    let index_bytes = type_index_bytes(program)?;
    reserve_builder_structure(index_bytes)?;
    let mut types = Vec::with_capacity(program.types.len());
    for declaration in &program.types {
        types.push((declaration.name.as_str(), declaration));
    }
    // Duplicate keys always refuse with the same diagnostic; equal-key order
    // is never consumed. Unstable sorting retains that verdict without a
    // hidden stable-sort scratch allocation.
    types.sort_unstable_by(|left, right| left.0.cmp(right.0));
    if types.windows(2).any(|items| items[0].0 == items[1].0) {
        return Err(vec![graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )]);
    }
    let mut generated = Vec::new();
    if !program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        reserve_builder_structure(std::mem::size_of::<Function>())?;
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        generated = Vec::with_capacity(1);
        generated.push(Function {
            stable_id: budgeted_format(format_args!("workspace.synthetic.main.{}", program.module)),
            explicit_id: true,
            name: crate::bounded_output::budgeted_clone("main"),
            name_span: Span::default(),
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::I64,
            effects: Vec::new(),
            yields: None,
            follows: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Int(0),
                span: Span::default(),
            },
            span: Span::default(),
        });
    }
    // Only a leaf Int body is owned, so ordinary field Drop allocates nothing
    // and never descends through an authored recursive subtree.
    SyntheticProgramView::new(program, generated)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

pub(in crate::workspace_graph) fn ast_peak(program: &Program) -> Result<usize, Vec<Diagnostic>> {
    let generated = if program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        0
    } else {
        synthetic_main_runtime_cost(&program.module)?.total
    };
    let owned = checked_usage(
        std::mem::size_of::<Input<'_>>(),
        generated,
        "builder_bytes",
        active_builder_limit(),
    )?;
    checked_usage(
        owned,
        type_index_bytes(program)?,
        "builder_bytes",
        active_builder_limit(),
    )
}
fn type_index_bytes(program: &Program) -> Result<usize, Vec<Diagnostic>> {
    program
        .types
        .len()
        .checked_mul(std::mem::size_of::<(&str, &TypeDeclaration)>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

/// Extra inline carrier over the ordinary owned Program. The resolved-core
/// invocation retains one reused input slot, never one header per module.
pub(in crate::workspace_graph) fn input_carrier_delta() -> Result<usize, Vec<Diagnostic>> {
    std::mem::size_of::<Input<'_>>()
        .checked_sub(std::mem::size_of::<Program>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

/// Hits/reuse candidates and invariant transforms keep the owned input.
/// First checked cold keys become owned only at the retention boundary.
// The reused inline owning fallback is priced by input_carrier_delta using
// actual sizeof; a Box would introduce a separate unpriced allocation.
#[allow(clippy::large_enum_variant)]
pub(in crate::workspace_graph) enum Input<'a> {
    Owned(Program),
    Borrowed(SyntheticProgramView<'a>),
}
impl<'a> Input<'a> {
    pub(in crate::workspace_graph) fn construct(
        program: &'a Program,
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        programs: &[Program],
        registry: &private_layout::registry::LayoutRegistry<'_>,
        uncached: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        if uncached && eligible(program) {
            construct(program).map(Self::Borrowed)
        } else {
            synthetic_program_with_registry(program, authored, programs, Some(registry))
                .map(Self::Owned)
        }
    }
    pub(in crate::workspace_graph) fn borrowed(&self) -> Option<&SyntheticProgramView<'_>> {
        match self {
            Self::Owned(_) => None,
            Self::Borrowed(program) => Some(program),
        }
    }

    pub(in crate::workspace_graph) fn into_checked_key(self) -> Result<Program, Vec<Diagnostic>> {
        match self {
            Self::Owned(program) => Ok(program),
            Self::Borrowed(view) => checked_key::materialize(view),
        }
    }

    pub(in crate::workspace_graph) fn owned(&self) -> Option<&Program> {
        match self {
            Self::Owned(program) => Some(program),
            Self::Borrowed(_) => None,
        }
    }
    pub(in crate::workspace_graph) fn source(
        &self,
    ) -> &dyn crate::ast::program_source::ProgramSource {
        match self {
            Self::Owned(program) => program,
            Self::Borrowed(program) => program,
        }
    }
    #[cfg(test)]
    pub(in crate::workspace_graph) fn into_owned(self) -> Result<Program, Vec<Diagnostic>> {
        match self {
            Self::Owned(program) => Ok(program),
            Self::Borrowed(_) => Err(vec![graph_error(
                "SPX-G173",
                "borrowed projection cannot become a checked cache key",
            )]),
        }
    }
    pub(in crate::workspace_graph) fn resolve<'s>(
        &'s self,
        private: Option<crate::source_verify::private_names::PrivateNames<'s>>,
    ) -> Result<(hir::ResolvedProgram, hir::FunctionResolutionWork), Vec<Diagnostic>> {
        match self {
            Self::Owned(program) => {
                hir::resolve_with_function_reuse(program, None, None, None, private)
            }
            Self::Borrowed(program) => hir::resolve_source(program, private),
        }
    }
}

#[cfg(test)]
#[path = "borrowed_leaf/tests.rs"]
mod tests;

#[path = "borrowed_leaf/checked_key.rs"]
mod checked_key;
