//! Explicit account storage receiver. The complete ordinary receiver remains
//! in the parent. Unfunded nominal admission refuses before acquisition.
use super::super::{allocation, ReplayBudget, MAX_REPLAY_WORK_UNITS};
use super::*;

pub(super) fn replay(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[ResolvedRecordMatchPatternField],
    mode: ResolvedMatchMode,
) -> Option<Result<ExpectedDestructure, Diagnostic>> {
    let lease = match allocation::authority(program, function) {
        Ok(None) => return None,
        Ok(Some(lease)) => lease,
        Err(_) => {
            return Some(Err(Diagnostic::io_static(
                "SPX-H006",
                "oracle invocation authority refused destructure replay",
            )))
        }
    };
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
    Some((|| {
        budget.prepare_builder(function)?;
        budget.prepare_clone_cursor(function)?;
        replay_paid(
            program,
            function,
            record,
            instance,
            fields,
            mode,
            &mut budget,
        )
    })())
}

fn insert_binding(
    bindings: &mut BTreeMap<Vec<usize>, (Vec<DeclarationId>, ValueId)>,
    ordinals: Vec<usize>,
    path: Vec<DeclarationId>,
    binding: ValueId,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<(Vec<DeclarationId>, ValueId)>, Diagnostic> {
    if !bindings.contains_key(&ordinals) {
        budget.reserve_btree_node::<Vec<usize>, (Vec<DeclarationId>, ValueId)>(
            function,
            "destructure binding node",
        )?;
    }
    Ok(bindings.insert(ordinals, (path, binding)))
}

fn replay_paid(
    program: &dyn crate::cleanup::ReplayContext,
    function: &crate::hir::ResolvedFunction,
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[ResolvedRecordMatchPatternField],
    mode: ResolvedMatchMode,
    budget: &mut super::super::ReplayBudget,
) -> Result<ExpectedDestructure, Diagnostic> {
    let mut queue = VecDeque::new();
    budget.reserve_deque(function, &mut queue, 1, "destructure pending root")?;
    queue.push_back(Pending {
        record,
        instance,
        fields,
        prefix: Vec::new(),
        ordinal_prefix: Vec::new(),
        depth: 1,
    });
    let mut nested = false;
    let mut field_work = 0usize;
    let mut bindings = BTreeMap::new();
    let mut non_byte_owned_terminal = false;

    while let Some(pending) = queue.pop_front() {
        budget.charge(function, 1, "destructure pending receive")?;
        if pending.depth > DEPTH_LIMIT {
            return Err(super::super::replay_error(
                function,
                "nested record destructure exceeds its replay depth bound",
            ));
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = pending.instance
        else {
            return Err(super::super::replay_error(
                function,
                "nested record destructure replay has a non-nominal instance",
            ));
        };
        if declaration != pending.record {
            return Err(super::super::replay_error(
                function,
                "nested record destructure replay changes record identity",
            ));
        }
        let declarations = program
            .declarations()
            .record_fields(pending.record)
            .ok_or_else(|| {
                super::super::replay_error(function, "nested record destructure has no declaration")
            })?;
        if declarations.len() != pending.fields.len() {
            return Err(super::super::replay_error(
                function,
                "nested record destructure replay is not exact",
            ));
        }
        let mut by_id = BTreeMap::new();
        for field in pending.fields {
            budget.charge(function, 1, "destructure field lookup construction")?;
            if !by_id.contains_key(&field.field) {
                budget.reserve_btree_node::<&DeclarationId, &ResolvedRecordMatchPatternField>(
                    function,
                    "destructure field lookup node",
                )?;
            }
            by_id.insert(&field.field, field);
        }
        if by_id.len() != declarations.len() {
            return Err(super::super::replay_error(
                function,
                "nested record destructure replay has duplicate fields",
            ));
        }
        for (ordinal, declaration) in declarations.iter().enumerate() {
            budget.charge(function, 1, "destructure declared field receive")?;
            field_work = field_work.checked_add(1).ok_or_else(|| {
                super::super::replay_error(function, "nested record destructure work overflow")
            })?;
            if field_work > FIELD_WORK_LIMIT {
                return Err(super::super::replay_error(
                    function,
                    "nested record destructure exceeds its replay field bound",
                ));
            }
            let field = by_id.get(&declaration.id).ok_or_else(|| {
                super::super::replay_error(function, "nested record destructure omits a field")
            })?;
            let ty = super::super::ordinary_inventory::substitute::substitute_type(
                &declaration.ty,
                pending.record,
                arguments,
                function,
                budget,
            )?;
            let needs_drop = super::super::type_needs_drop(program, function, &ty)?;
            budget.reserve_clone(function, &pending.prefix, "destructure path clone")?;
            let mut path = pending.prefix.clone();
            budget.reserve_vec(function, &mut path, 1, "destructure path extension")?;
            path.push(declaration.id.clone());
            budget.reserve_clone(
                function,
                &pending.ordinal_prefix,
                "destructure ordinal clone",
            )?;
            let mut ordinal_path = pending.ordinal_prefix.clone();
            budget.reserve_vec(
                function,
                &mut ordinal_path,
                1,
                "destructure ordinal extension",
            )?;
            ordinal_path.push(ordinal);
            match &field.pattern {
                ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    let ownership = if needs_drop {
                        match mode {
                            ResolvedMatchMode::Own => OwnershipMode::Own,
                            ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                            ResolvedMatchMode::Value => OwnershipMode::Value,
                        }
                    } else {
                        OwnershipMode::Value
                    };
                    if binding.ty != ty || binding.ownership != ownership {
                        return Err(super::super::replay_error(
                            function,
                            "nested record destructure replay binding is not canonical",
                        ));
                    }
                    // The nominal whole-subtree predicate has its own owning
                    // traversal, which is not prepaid by this storage receiver.
                    // Refuse visibly before calling it; never infer its verdict.
                    if needs_drop
                        && mode == ResolvedMatchMode::Own
                        && matches!(&ty, ResolvedType::Nominal { .. })
                    {
                        budget
                            .oracle_lease()
                            .expect("explicit payer")
                            .fail_physical(
                                "destructure nominal admission acquisition is not funded",
                            );
                        return Err(budget.budget_refusal(
                            function,
                            "destructure nominal admission acquisition is not funded",
                            "",
                        ));
                    }
                    let subtree = needs_drop
                        && mode == ResolvedMatchMode::Own
                        && crate::hir::owned_record_subtree::admitted(
                            program.declarations(),
                            instance,
                            &ty,
                        );
                    nested |= subtree;
                    non_byte_owned_terminal |= needs_drop
                        && !subtree
                        && !matches!(ty, ResolvedType::Bytes | ResolvedType::String)
                        && !crate::hir::owned_collection_record::vector(
                            program.declarations(),
                            &ty,
                        );
                    if needs_drop
                        && mode == ResolvedMatchMode::Own
                        && (bindings.len() >= OWNED_LEAF_LIMIT
                            || insert_binding(
                                &mut bindings,
                                ordinal_path,
                                path,
                                binding.id.clone(),
                                function,
                                budget,
                            )?
                            .is_some())
                    {
                        return Err(super::super::replay_error(
                            function,
                            "nested record destructure replay leaf inventory is invalid",
                        ));
                    }
                }
                ResolvedRecordMatchFieldPattern::Wildcard => {
                    if needs_drop && mode == ResolvedMatchMode::Own {
                        return Err(super::super::replay_error(
                            function,
                            "nested owned record destructure replay drops a live field",
                        ));
                    }
                }
                ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => {
                    nested = true;
                    if instance != &ty {
                        return Err(super::super::replay_error(
                            function,
                            "nested record destructure replay instance is inconsistent",
                        ));
                    }
                    budget.reserve_deque(function, &mut queue, 1, "destructure nested pending")?;
                    queue.push_back(Pending {
                        record,
                        instance,
                        fields,
                        prefix: path,
                        ordinal_prefix: ordinal_path,
                        depth: pending.depth + 1,
                    });
                }
            }
        }
    }
    if nested && non_byte_owned_terminal {
        return Err(super::super::replay_error(
            function,
            "exact nested record destructure replay did not reach owned Bytes leaves",
        ));
    }
    budget.reserve_array::<ExpectedBinding>(
        function,
        bindings.len(),
        "destructure final bindings",
    )?;
    Ok(ExpectedDestructure {
        nested,
        bindings: bindings
            .into_iter()
            .map(|(_, (path, binding))| ExpectedBinding { path, binding })
            .collect(),
    })
}

#[cfg(test)]
mod tests;
