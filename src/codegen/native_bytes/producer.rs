//! Shared producer algorithms over an explicit borrowed cleanup context.
use super::*;
pub(in crate::codegen) trait NativeBytesProducer: Sized {
    fn base(&self) -> &NativeBytesPlan;
    fn transitions(&self) -> origin::Selection<'_, ExpressionId>;
    fn variant_cases(&self) -> origin::Selection<'_, (ExpressionId, DeclarationId)>;
    fn authenticate_literal_format_commit(
        &self,
        call: &ExpressionId,
        args: &[crate::hir::ResolvedExpr],
    ) -> Result<(), Diagnostic> {
        let mut commits = self
            .transitions()
            .get(call)
            .into_iter()
            .flatten()
            .filter_map(|transition| match transition {
                CleanupTransition::CallCommit {
                    call: owner,
                    arguments,
                } if owner == call => Some(arguments),
                _ => None,
            });
        let arguments = commits
            .next()
            .ok_or_else(|| error("literal format lacks canonical CallCommit"))?;
        if commits.next().is_some() {
            return Err(error("literal format has duplicate CallCommit"));
        }
        let expected = args
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.ty == crate::hir::ResolvedType::String)
            .collect::<Vec<_>>();
        if arguments.len() != expected.len() {
            return Err(error("literal format CallCommit ownership disagrees"));
        }
        for (transfer, (index, argument)) in arguments.iter().zip(expected) {
            if transfer.parameter_index != index as u32
                || !transfer.source.projections.is_empty()
                || !matches!(&transfer.source.storage,
                    StorageId::CallArgument { call: owner, parameter_index, value_expression }
                    if owner == call && *parameter_index == index as u32 && value_expression == &argument.id)
            {
                return Err(error(
                    "literal format CallCommit is not source-authenticated",
                ));
            }
        }
        Ok(())
    }

    fn apply_at(&self, at: &ExpressionId) -> Result<String, Diagnostic> {
        self.apply_transitions(self.transitions().get(at).into_iter().flatten())
    }
    fn apply_transitions<'a>(
        &self,
        transitions: impl Iterator<Item = &'a CleanupTransition>,
    ) -> Result<String, Diagnostic> {
        let mut output = String::new();
        for transition in transitions {
            match transition {
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                }
                | CleanupTransition::Renew {
                    source,
                    destination,
                    ..
                } => {
                    for (source, destination) in self.base().transfer_pairs(source, destination)? {
                        output.push_str(
                            &self.base().release_replaced_string(transition, destination),
                        );
                        output.push_str(&emit_transfer(source, destination, "plan transfer"));
                    }
                }
                CleanupTransition::Initialize { destination, .. } => {
                    if destination.projections.is_empty()
                        && self.base().has_variant_leaves(&destination.storage)
                    {
                        // A variant root is conditionally initialized from its
                        // authenticated runtime tag by the carrier bridge.
                        continue;
                    }
                    for place in self.base().leaves_under(destination)? {
                        let destination = &self.base().slots[place];
                        output.push_str(&format!(
                            "if ({}) spx_runtime_invariant_failure(\"Bytes plan initialize liveness\");\n{} = true;\n",
                            destination.flag, destination.flag
                        ));
                    }
                }
                CleanupTransition::ReserveRenewal { .. }
                | CleanupTransition::CallCommit { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
            }
        }
        Ok(output)
    }
    fn apply_variant_case_at(
        &self,
        at: &ExpressionId,
        case: &DeclarationId,
    ) -> Result<String, Diagnostic> {
        self.apply_variant_case_at_inner(at, case, true)
    }
    fn apply_try_variant_case_at(
        &self,
        at: &ExpressionId,
        case: &DeclarationId,
        transfer_residual: bool,
    ) -> Result<String, Diagnostic> {
        self.apply_variant_case_at_inner(at, case, transfer_residual)
    }
    fn apply_variant_case_at_inner(
        &self,
        at: &ExpressionId,
        case: &DeclarationId,
        include_variant_transfer: bool,
    ) -> Result<String, Diagnostic> {
        nested_owned::apply_variant_case_at(self, at, case, include_variant_transfer)
    }
    fn authenticate_variant_case_at(
        &self,
        at: &ExpressionId,
        selected: &DeclarationId,
    ) -> Result<String, Diagnostic> {
        nested_owned::authenticate_variant_case_at(self, at, selected)
    }
    fn apply_variant_at(
        &self,
        at: &ExpressionId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let mut output = self.apply_at(at)?;
        let variant_transitions = self
            .transitions()
            .get(at)
            .into_iter()
            .flatten()
            .filter_map(|transition| match transition {
                CleanupTransition::TransferVariant {
                    source,
                    destination,
                    variant,
                    ..
                } => Some((source, destination, variant)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let alternative_destinations = variant_transitions.iter().fold(
            BTreeMap::<&CleanupPlace, usize>::new(),
            |mut counts, row| {
                *counts.entry(row.1).or_default() += 1;
                counts
            },
        );
        for (source, destination, variant) in variant_transitions {
            // Variant-producing `if` branches are transferred while each
            // branch is still selected. Reapplying their joined alternatives
            // here would demand that both mutually exclusive sources be live.
            if alternative_destinations[&destination] > 1 {
                continue;
            }
            if variant != &layout.variant {
                return Err(error("variant transfer identity disagrees with layout"));
            }
            output.push_str(&self.base().emit_variant_transfer(
                source,
                destination,
                carrier,
                layout,
            )?);
        }
        Ok(output)
    }
    /// One independently staged variant transfer phase. A repeated expression
    /// anchor may bracket a scope exit; destination identity selects the exact
    /// canonical transition without applying its later phase early.
    fn apply_variant_into_at(
        &self,
        at: &ExpressionId,
        carrier: &str,
        layout: &VariantLayout,
        destination_storage: &StorageId,
    ) -> Result<String, Diagnostic> {
        let mut found =
            self.transitions()
                .get(at)
                .into_iter()
                .flatten()
                .filter_map(|row| match row {
                    CleanupTransition::TransferVariant {
                        source,
                        destination,
                        variant,
                        ..
                    } if &destination.storage == destination_storage => {
                        Some((source, destination, variant))
                    }
                    _ => None,
                });
        let (source, destination, variant) = found
            .next()
            .ok_or_else(|| error("variant phase has no exact canonical destination"))?;
        if found.next().is_some() || variant != &layout.variant {
            return Err(error(
                "variant phase destination is ambiguous or has wrong layout",
            ));
        }
        self.base()
            .emit_variant_transfer(source, destination, carrier, layout)
    }
    fn result_at(&self, at: &ExpressionId) -> Option<&str> {
        result_at(self.base(), self.transitions(), at)
    }
    fn apply_try_success_at(
        &self,
        at: &ExpressionId,
        case: &DeclarationId,
    ) -> Result<(String, Option<&str>), Diagnostic> {
        let transitions = self
            .variant_cases()
            .get(&(at.clone(), case.clone()))
            .ok_or_else(|| error("owned Result success has no authenticated cleanup block"))?;
        let output = self.apply_transitions(transitions.iter())?;
        let result = try_success_result_at(self.base(), self.variant_cases(), at, case);
        Ok((output, result))
    }
    fn transfer_field_at(
        &self,
        at: &ExpressionId,
        source_value: &str,
        destination: &CleanupPlace,
    ) -> Result<String, Diagnostic> {
        let mut matches =
            self.transitions()
                .get(at)
                .into_iter()
                .flatten()
                .filter_map(|transition| match transition {
                    CleanupTransition::Transfer {
                        source,
                        destination: target,
                        ..
                    }
                    | CleanupTransition::Renew {
                        source,
                        destination: target,
                        ..
                    } if target == destination => Some(source),
                    _ => None,
                });
        let source = matches
            .next()
            .ok_or_else(|| error("owned Bytes field has no canonical initializer transfer"))?;
        if matches.next().is_some() {
            return Err(error("owned Bytes field initializer transfer is ambiguous"));
        }
        let source = self
            .base()
            .slots
            .get(source)
            .ok_or_else(|| error("owned Bytes field source is not indexed"))?;
        let destination = self
            .base()
            .slots
            .get(destination)
            .ok_or_else(|| error("owned Bytes field destination is not indexed"))?;
        // Calls and blocks may already have applied this exact transition.
        // Places and branch values leave it to their consuming constructor.
        if source_value == destination.value {
            return Ok(String::new());
        }
        if source_value != source.value {
            return Err(error(
                "owned Bytes field value disagrees with its plan source",
            ));
        }
        Ok(emit_transfer(
            source,
            destination,
            "field initializer transfer",
        ))
    }
    fn transfer_scalar_match_arm(
        &self,
        match_id: &ExpressionId,
        arm_value_id: &ExpressionId,
    ) -> Result<String, Diagnostic> {
        let source_storage = crate::cleanup_plan::StorageId::Temporary(arm_value_id.clone());
        let dest_storage = crate::cleanup_plan::StorageId::Temporary(match_id.clone());
        let mut matches = self
            .transitions()
            .get(match_id)
            .into_iter()
            .flatten()
            .filter_map(|transition| match transition {
                CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                }
                | CleanupTransition::Renew {
                    source,
                    destination,
                    ..
                } if source.storage == source_storage && destination.storage == dest_storage => {
                    Some((source, destination))
                }
                _ => None,
            });
        let (source, destination) = matches.next().ok_or_else(|| {
            error(format!(
                "scalar match arm `{arm_value_id}` has no canonical transfer to match `{match_id}`"
            ))
        })?;
        if matches.next().is_some() {
            return Err(error("scalar match arm transfer is ambiguous"));
        }
        let source_slot = self
            .base()
            .slots
            .get(source)
            .ok_or_else(|| error("scalar match arm source is not indexed"))?;
        let dest_slot = self
            .base()
            .slots
            .get(destination)
            .ok_or_else(|| error("scalar match destination is not indexed"))?;
        Ok(super::emit_transfer(
            source_slot,
            dest_slot,
            "scalar match arm transfer",
        ))
    }

    fn transfer_branch_at(
        &self,
        at: &ExpressionId,
        source_value: &str,
        destination: &CleanupPlace,
    ) -> Result<String, Diagnostic> {
        // Branch joins are anchored at the parent expression. Select only
        // the reached branch's exact source/destination relation; never replay
        // the other branch transitions sharing that parent anchor.
        let mut matches =
            self.transitions()
                .get(at)
                .into_iter()
                .flatten()
                .filter_map(|transition| match transition {
                    CleanupTransition::Transfer {
                        source,
                        destination: target,
                        ..
                    }
                    | CleanupTransition::Renew {
                        source,
                        destination: target,
                        ..
                    } if target == destination => {
                        let slot = self.base().slots.get(source)?;
                        (slot.value == source_value).then_some(source)
                    }
                    _ => None,
                });
        let source = matches
            .next()
            .ok_or_else(|| error("Bytes branch source has no canonical result transfer"))?;
        if matches.next().is_some() {
            return Err(error("Bytes branch transfer is ambiguous"));
        }
        let source = self
            .base()
            .slots
            .get(source)
            .ok_or_else(|| error("Bytes branch source is not indexed"))?;
        let destination = self
            .base()
            .slots
            .get(destination)
            .ok_or_else(|| error("Bytes branch destination is not indexed"))?;
        Ok(emit_transfer(source, destination, "branch transfer"))
    }
    fn initialize_record_result_at(
        &self,
        at: &ExpressionId,
        carrier: &str,
    ) -> Result<String, Diagnostic> {
        let mut destinations =
            self.transitions()
                .get(at)
                .into_iter()
                .flatten()
                .filter_map(|transition| match transition {
                    CleanupTransition::Initialize { destination, .. }
                        if !destination.projections.is_empty()
                            || self
                                .base()
                                .storage_leaves
                                .get(&destination.storage)
                                .is_some_and(|leaves| {
                                    leaves.iter().any(|leaf| !leaf.projections.is_empty())
                                }) =>
                    {
                        Some(destination)
                    }
                    _ => None,
                });
        let destination = destinations
            .next()
            .ok_or_else(|| error("owned record result has no canonical initialization"))?;
        if destinations.next().is_some() {
            return Err(error("owned record result initialization is ambiguous"));
        }
        let mut output = String::new();
        for place in self.base().leaves_under(destination)? {
            let relative = &place.projections[destination.projections.len()..];
            let path = nested_owned::c_field_path(relative)?;
            let slot = &self.base().slots[place];
            output.push_str(&format!(
                "{} = {};\n",
                slot.value,
                slot.kind.move_call(&format!("(({carrier}).{path})")),
            ));
        }
        Ok(output)
    }

    fn transfer_to(&self, storage: &StorageId, at: &ExpressionId) -> Result<String, Diagnostic> {
        let mut matches =
            self.transitions()
                .get(at)
                .into_iter()
                .flatten()
                .filter_map(|transition| match transition {
                    CleanupTransition::Transfer {
                        source,
                        destination,
                        ..
                    }
                    | CleanupTransition::Renew {
                        source,
                        destination,
                        ..
                    } => (destination.storage == *storage && destination.projections.is_empty())
                        .then_some((source, destination)),
                    _ => None,
                });
        let Some((source, destination)) = matches.next() else {
            return Err(error(format!(
                "Bytes destination `{storage:?}` has no canonical transfer"
            )));
        };
        if matches.next().is_some() {
            return Err(error(format!(
                "Bytes destination `{storage:?}` has ambiguous transfers"
            )));
        }
        let source = self
            .base()
            .slots
            .get(source)
            .ok_or_else(|| error("Bytes transfer source is not indexed"))?;
        let destination = self
            .base()
            .slots
            .get(destination)
            .ok_or_else(|| error("Bytes transfer destination is not indexed"))?;
        if source.kind != destination.kind {
            return Err(error("owned plan transfer changes carrier kind"));
        }
        let release = self.transitions().get(at).into_iter().flatten()
            .filter(|t| matches!(t, CleanupTransition::Renew { destination: d, .. } if d.storage == *storage && d.projections.is_empty()))
            .map(|t| self.base().release_replaced_string(t, destination)).collect::<String>();
        Ok(release + &format!(
            "if (!{} || {}) spx_runtime_invariant_failure(\"owned plan transfer liveness {} to {}\");\n{} = {};\n{} = false;\n{} = true;\n",
            source.flag,
            destination.flag,
            source.value,
            destination.value,
            destination.value,
            source.kind.move_call(&source.value),
            source.flag,
            destination.flag
        ))
    }
    fn apply_variant_match_destructure_at(
        &self,
        expression: &ExpressionId,
        case: &DeclarationId,
    ) -> Result<String, Diagnostic> {
        // Match-keyed TransferVariant rows are post-arm continuations. Only
        // selected scrutinee-field transfers belong before the arm body.
        self.apply_variant_case_at_inner(expression, case, false)
    }

    fn apply_variant_join_continuation(
        &self,
        expression: &ExpressionId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let mut source = CleanupPlace {
            storage: StorageId::Temporary(expression.clone()),
            projections: Vec::new(),
        };
        let mut output = String::new();
        for transition in self.transitions().get(expression).into_iter().flatten() {
            if let CleanupTransition::TransferVariant {
                source: candidate,
                destination,
                variant,
                ..
            } = transition
            {
                if *candidate != source {
                    continue;
                }
                if *variant != layout.variant {
                    return Err(error("variant join continuation identity differs"));
                }
                output.push_str(&self.base().emit_variant_transfer(
                    candidate,
                    destination,
                    carrier,
                    layout,
                )?);
                source = destination.clone();
            }
        }
        Ok(output)
    }
    fn apply_variant_if_branch(
        &self,
        parent: &ExpressionId,
        branch: &crate::hir::ResolvedExpr,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let source = match &branch.kind {
            crate::hir::ResolvedExprKind::Place(place) => {
                let mut projections = Vec::new();
                for projection in &place.projections {
                    let crate::hir::PlaceProjection::Field(field) = projection else {
                        return Err(error("owning variant If place has unsupported projection"));
                    };
                    projections.push(field.clone());
                }
                CleanupPlace {
                    storage: StorageId::Value(place.root.clone()),
                    projections,
                }
            }
            _ => CleanupPlace {
                storage: StorageId::Temporary(branch.id.clone()),
                projections: Vec::new(),
            },
        };
        let destination = CleanupPlace {
            storage: StorageId::Temporary(parent.clone()),
            projections: Vec::new(),
        };
        let count = self.transitions().get(parent).into_iter().flatten().filter(|transition| matches!(transition,
            CleanupTransition::TransferVariant { source: actual_source, destination: actual_destination, variant, .. }
            if *actual_source == source && *actual_destination == destination && *variant == layout.variant)).count();
        if count != 1 {
            return Err(error(
                "owning variant If has no unique canonical branch transfer",
            ));
        }
        self.base()
            .emit_variant_transfer(&source, &destination, carrier, layout)
    }
    fn apply_record_if_branch(
        &self,
        parent: &ExpressionId,
        branch: &ExpressionId,
    ) -> Result<String, Diagnostic> {
        let destination = CleanupPlace {
            storage: StorageId::Temporary(parent.clone()),
            projections: Vec::new(),
        };
        let source_storage = StorageId::Temporary(branch.clone());
        let transitions = self
            .transitions()
            .get(parent)
            .ok_or_else(|| error("owning record If has no canonical transitions"))?;
        let selected = transitions.iter().filter(|transition| matches!(transition,
            CleanupTransition::Transfer { source, destination: target, .. }
                | CleanupTransition::Renew { source, destination: target, .. }
                if *target == destination && source.storage == source_storage && source.projections.is_empty())).count();
        if selected != 1 {
            return Err(error(
                "owning record If branch has no unique canonical join",
            ));
        }
        let mut output = String::new();
        for transition in transitions {
            match transition {
                CleanupTransition::Transfer {
                    source,
                    destination: target,
                    ..
                }
                | CleanupTransition::Renew {
                    source,
                    destination: target,
                    ..
                } => {
                    if *target == destination && source.storage != source_storage {
                        continue;
                    }
                    for (source, target) in self.base().transfer_pairs(source, target)? {
                        output.push_str(&emit_transfer(source, target, "record If plan transfer"));
                    }
                }
                CleanupTransition::ReserveRenewal { .. }
                | CleanupTransition::Initialize { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::StageCopyResult { .. } => {
                    return Err(error("owning record If has a non-record continuation"));
                }
                CleanupTransition::CallCommit { .. } | CleanupTransition::SelectFailure { .. } => {}
            }
        }
        Ok(output)
    }
    fn authenticate_transfers_at(&self, at: &ExpressionId) -> Result<String, Diagnostic> {
        nested_owned::authenticate_transfers_at(self, at, None)
    }
    fn record_match_phase(
        &self,
        at: &ExpressionId,
        bindings: &BTreeSet<StorageId>,
        entering: bool,
        authenticate_only: bool,
    ) -> Result<String, Diagnostic> {
        if authenticate_only {
            return nested_owned::authenticate_transfers_at(self, at, Some((bindings, entering)));
        }
        self.apply_transitions(self.transitions().get(at).into_iter().flatten().filter(
            |transition| nested_owned::record_match_entry(transition, bindings) == entering,
        ))
    }
    fn initialize_variant_result_at(
        &self,
        at: &ExpressionId,
        carrier: &str,
        layout: &VariantLayout,
    ) -> Result<String, Diagnostic> {
        let mut destinations =
            self.transitions()
                .get(at)
                .into_iter()
                .flatten()
                .filter_map(|transition| match transition {
                    CleanupTransition::InitializeVariant {
                        destination,
                        variant,
                        ..
                    } if destination.projections.is_empty()
                        && self.base().has_variant_leaves(&destination.storage)
                        && variant == &layout.variant =>
                    {
                        Some(destination)
                    }
                    _ => None,
                });
        let destination = destinations
            .next()
            .ok_or_else(|| error("owned variant result has no canonical initialization"))?;
        if destinations.next().is_some() {
            return Err(error("owned variant result initialization is ambiguous"));
        }
        let mut output = format!(
            "if (({carrier}).spx_tag >= UINT32_C({})) spx_runtime_invariant_failure(\"invalid owned variant result tag\");\n",
            layout.cases.len()
        );
        for place in self.base().leaves_under(destination)? {
            let [case_id, field_id, ..] = place.projections.as_slice() else {
                return Err(error(
                    "owned variant result Bytes leaf is not case-qualified",
                ));
            };
            let case = layout
                .case(case_id)
                .ok_or_else(|| error("owned variant result case disagrees with layout"))?;
            if case.field(field_id).is_none() {
                return Err(error("owned variant result field disagrees with layout"));
            }
            let slot = &self.base().slots[place];
            output.push_str(&format!(
                "if (({carrier}).spx_tag == UINT32_C({})) {{\n    if ({}) spx_runtime_invariant_failure(\"owned variant result leaf already live\");\n    {} = {};\n    {} = true;\n}} else if ({}) spx_runtime_invariant_failure(\"inactive owned variant result leaf is live\");\n",
                case.tag,
                slot.flag,
                slot.value,
                slot.kind.move_call(&format!("(({carrier}).spx_payload.{}.{})",c_case_symbol(case_id),nested_owned::variant_field_path(case, &place.projections[1..])?)),
                slot.flag,
                slot.flag,
            ));
        }
        Ok(output)
    }
}
pub(super) fn result_at<'a>(
    base: &'a NativeBytesPlan,
    selection: origin::Selection<'_, ExpressionId>,
    at: &ExpressionId,
) -> Option<&'a str> {
    selection.get(at).and_then(|transitions| {
        transitions
            .iter()
            .rev()
            .find_map(|transition| match transition {
                CleanupTransition::Initialize { destination, .. }
                | CleanupTransition::Transfer { destination, .. }
                | CleanupTransition::Renew { destination, .. } => {
                    base.slots.get(destination).map(|slot| slot.value.as_str())
                }
                CleanupTransition::ReserveRenewal { .. }
                | CleanupTransition::CallCommit { .. }
                | CleanupTransition::InitializeVariant { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => None,
            })
    })
}

pub(super) fn try_success_result_at<'a>(
    base: &'a NativeBytesPlan,
    selection: origin::Selection<'_, (ExpressionId, DeclarationId)>,
    at: &ExpressionId,
    case: &DeclarationId,
) -> Option<&'a str> {
    selection
        .get(&(at.clone(), case.clone()))
        .and_then(|transitions| {
            transitions
                .iter()
                .rev()
                .find_map(nested_owned::transition_destination)
                .and_then(|destination| base.slots.get(destination))
                .map(|slot| slot.value.as_str())
        })
}
