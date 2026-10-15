//! Exact lifecycle lookup for the cleanup control-flow oracle.
use super::*;

pub(super) fn preflight_finalizer_bindings(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    scenario: &CleanupScenario,
    checked_allocation: bool,
) -> Result<BTreeMap<DeclarationId, Option<DeclarationId>>, CleanupExecutionError> {
    let known_imports = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .map(|import| import.id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = scenario
        .available_finalizer_imports
        .iter()
        .find(|import| !known_imports.contains(*import))
    {
        return Err(CleanupExecutionError::UnknownFinalizerBinding(
            unknown.clone(),
        ));
    }

    let mut bindings = BTreeMap::new();
    for action in function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
    {
        if bindings.contains_key(&action.lifecycle_id) {
            continue;
        }
        let binding = if checked_allocation
            && function.cleanup_plan.schema == super::super::CLEANUP_PLAN_SCHEMA_V20
            && action.lifecycle_id.as_str() == "core.vec.drop"
        {
            // The private caller replayed the exact current v20 owner before
            // constructing this oracle. This is simulated settlement only.
            None
        } else {
            resolve_lifecycle_binding(program, &action.lifecycle_id)?
        };
        if let Some(import) = &binding {
            if !scenario.available_finalizer_imports.contains(import) {
                return Err(CleanupExecutionError::MissingFinalizerBinding(
                    import.clone(),
                ));
            }
        }
        bindings.insert(action.lifecycle_id.clone(), binding);
    }
    Ok(bindings)
}

fn resolve_lifecycle_binding(
    program: &ResolvedProgram,
    lifecycle: &DeclarationId,
) -> Result<Option<DeclarationId>, CleanupExecutionError> {
    if matches!(
        lifecycle.as_str(),
        "core.bytes.drop" | "core.string.drop" | "core.fn_once.drop" | "core.fn_once_i64.drop.v2"
    ) {
        return Ok(None);
    }
    let mut binding = None;
    for declaration in &program.types {
        let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
            continue;
        };
        if drop.id != *lifecycle {
            continue;
        }
        if binding.is_some() {
            return Err(invariant(format!(
                "lifecycle `{lifecycle}` resolves more than once"
            )));
        }
        binding = Some(match &drop.kind {
            ResolvedResourceDropKind::Trivial => None,
            ResolvedResourceDropKind::Imported { import, .. } => Some(import.clone()),
        });
    }
    binding.ok_or_else(|| invariant(format!("unknown lifecycle `{lifecycle}`")))
}

// Ordinary public and older test oracles preserve their lifecycle whitelist.
impl<'a> Executor<'a> {
    pub(super) fn new(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        scenario: CleanupScenario,
    ) -> Result<Self, CleanupExecutionError> {
        Self::new_with_lifecycle_policy(program, function, scenario, false)
    }
}

#[cfg(test)]
mod checked_vec_tests {
    use super::*;

    #[test]
    fn checked_vec_settlement_keeps_ordinary_and_non_v20_lifecycle_refusals() {
        let source = crate::parse(
            r#"module checked.lifecycle;
@id("checked.lifecycle.reserve") fn reserve()->usize {
 let values=vec_with_capacity<string>(1usize);
 let reserved=vec_reserve_owned<string>(values,1usize);
 vec_capacity<string>(reserved)
}
@id("checked.lifecycle.main") fn main()->i64 {0}
"#,
            "checked-vec-lifecycle.spx",
        )
        .unwrap();
        let entry = DeclarationId::new("checked.lifecycle.reserve");
        let prepared = crate::cleanup_plan::checked_allocation::prepare(&source, &entry).unwrap();
        let owner = prepared.receive().unwrap();
        owner.replay().unwrap();
        let program = owner.program();
        let function = program.functions.iter().find(|f| f.id == entry).unwrap();
        let scenario = CleanupScenario::new("checked-vec-bindings", None);
        let bindings = preflight_finalizer_bindings(program, function, &scenario, true).unwrap();
        assert_eq!(
            bindings.get(&DeclarationId::new("core.vec.drop")),
            Some(&None)
        );
        assert!(matches!(
            preflight_finalizer_bindings(program, function, &scenario, false),
            Err(CleanupExecutionError::HarnessInvariant(message))
                if message == "unknown lifecycle `core.vec.drop`"
        ));
        let mut ordinary = function.clone();
        ordinary.cleanup_plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19;
        assert!(preflight_finalizer_bindings(program, &ordinary, &scenario, true).is_err());
        assert!(resolve_lifecycle_binding(program, &DeclarationId::new("forged.drop")).is_err());
    }
}
