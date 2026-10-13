//! Complete original capacity census and aggregate feedback control.
#![allow(
    dead_code,
    reason = "retain complete original metric helpers for independent source and owning controls"
)]
use super::super::*;
#[cfg(test)]
thread_local! {
    static LOWER_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_lower_capacity_high_water() {
    LOWER_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
pub(crate) fn lower_capacity_high_water() -> usize {
    LOWER_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn note_lower_capacity_high_water(bytes: usize) {
    LOWER_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
fn storage_id_owned_capacity(storage: &StorageId) -> usize {
    match storage {
        StorageId::Value(id) => id.as_str().len(),
        StorageId::Temporary(id) => id.as_str().len(),
        StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => call.as_str().len() + value_expression.as_str().len(),
        StorageId::ProvisionalResult => 0,
    }
}

#[cfg(test)]
fn cleanup_place_owned_capacity(place: &CleanupPlace) -> usize {
    storage_id_owned_capacity(&place.storage)
        + place.projections.capacity() * std::mem::size_of::<DeclarationId>()
        + place
            .projections
            .iter()
            .map(|projection| projection.as_str().len())
            .sum::<usize>()
}

#[cfg(test)]
fn status_source_id_owned_capacity(source: &StatusSourceId) -> usize {
    source.expression.as_str().len()
}

#[cfg(test)]
fn field_shape_owned_capacity(shape: &FieldLivenessShape) -> usize {
    match shape {
        FieldLivenessShape::NoDrop => 0,
        FieldLivenessShape::Leaf { lifecycle, .. } => lifecycle.as_str().len(),
        FieldLivenessShape::Record {
            declaration,
            fields,
        } => {
            declaration.as_str().len()
                + fields.capacity() * std::mem::size_of::<FieldLiveness>()
                + fields
                    .iter()
                    .map(|field| {
                        field.field.as_str().len() + field_shape_owned_capacity(&field.shape)
                    })
                    .sum::<usize>()
        }
        FieldLivenessShape::Variant { declaration, cases } => {
            declaration.as_str().len()
                + cases.capacity() * std::mem::size_of::<crate::cleanup::VariantCaseLiveness>()
                + cases
                    .iter()
                    .map(|case| {
                        case.case.as_str().len()
                            + case.fields.capacity() * std::mem::size_of::<FieldLiveness>()
                            + case
                                .fields
                                .iter()
                                .map(|field| {
                                    field.field.as_str().len()
                                        + field_shape_owned_capacity(&field.shape)
                                })
                                .sum::<usize>()
                    })
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
fn cleanup_slot_owned_capacity(slot: &CleanupSlot) -> usize {
    storage_id_owned_capacity(&slot.storage)
        + resolved_type_owned_capacity(&slot.ty)
        + field_shape_owned_capacity(&slot.field_liveness_shape)
}

#[cfg(test)]
fn transition_owned_capacity(transition: &CleanupTransition) -> usize {
    match transition {
        CleanupTransition::Initialize { at, destination }
        | CleanupTransition::ReserveRenewal {
            at,
            binding: destination,
        } => at.as_str().len() + cleanup_place_owned_capacity(destination),
        CleanupTransition::InitializeVariant {
            at,
            destination,
            variant,
        } => at.as_str().len() + cleanup_place_owned_capacity(destination) + variant.as_str().len(),
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        }
        | CleanupTransition::Renew {
            at,
            source,
            destination,
        } => {
            at.as_str().len()
                + cleanup_place_owned_capacity(source)
                + cleanup_place_owned_capacity(destination)
        }
        CleanupTransition::TransferVariant {
            at,
            source,
            destination,
            variant,
        } => {
            at.as_str().len()
                + cleanup_place_owned_capacity(source)
                + cleanup_place_owned_capacity(destination)
                + variant.as_str().len()
        }
        CleanupTransition::AuthenticateVariantCase {
            at,
            source,
            variant,
            case,
        } => {
            at.as_str().len()
                + cleanup_place_owned_capacity(source)
                + variant.as_str().len()
                + case.as_str().len()
        }
        CleanupTransition::CallCommit { call, arguments } => {
            call.as_str().len()
                + arguments.capacity() * std::mem::size_of::<CallArgumentTransfer>()
                + arguments
                    .iter()
                    .map(|argument| cleanup_place_owned_capacity(&argument.source))
                    .sum::<usize>()
        }
        CleanupTransition::SelectFailure { source } => status_source_id_owned_capacity(source),
        CleanupTransition::StageCopyResult { source } => match source {
            StagedCopyResultSource::Body {
                expression,
                instance,
            } => expression.as_str().len() + resolved_type_owned_capacity(instance),
            StagedCopyResultSource::TryResidual {
                expression,
                operand,
                source_instance,
                target_instance,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
            } => {
                expression.as_str().len()
                    + operand.as_str().len()
                    + resolved_type_owned_capacity(source_instance)
                    + resolved_type_owned_capacity(target_instance)
                    + result.as_str().len()
                    + ok_case.as_str().len()
                    + ok_field.as_str().len()
                    + err_case.as_str().len()
                    + err_field.as_str().len()
            }
            StagedCopyResultSource::TryOptionNone {
                expression,
                operand,
                source_instance,
                target_instance,
                option,
                some_case,
                some_field,
                none_case,
            } => {
                expression.as_str().len()
                    + operand.as_str().len()
                    + resolved_type_owned_capacity(source_instance)
                    + resolved_type_owned_capacity(target_instance)
                    + option.as_str().len()
                    + some_case.as_str().len()
                    + some_field.as_str().len()
                    + none_case.as_str().len()
            }
        },
    }
}

#[cfg(test)]
fn edge_condition_owned_capacity(condition: &EdgeCondition) -> usize {
    match condition {
        EdgeCondition::Always => 0,
        EdgeCondition::BooleanResult(expression, _) => expression.as_str().len(),
        EdgeCondition::VariantCase {
            scrutinee, case, ..
        } => scrutinee.as_str().len() + case.as_str().len(),
        EdgeCondition::ArmSelected { scrutinee, .. } => scrutinee.as_str().len(),
        EdgeCondition::StatusZero(source) | EdgeCondition::StatusNonzero(source) => {
            status_source_id_owned_capacity(source)
        }
    }
}

#[cfg(test)]
fn exit_continuation_owned_capacity(continuation: &ExitContinuation) -> usize {
    lexical_control::exit_capacity(continuation)
}

#[cfg(test)]
pub(super) fn builder_nested_capacity(builder: &PlanBuilder<'_>) -> usize {
    let block_payload = builder.blocks.iter().fold(0usize, |bytes, block| {
        bytes
            + block.transitions.capacity() * std::mem::size_of::<CleanupTransition>()
            + block
                .transitions
                .iter()
                .map(transition_owned_capacity)
                .sum::<usize>()
            + match &block.terminator {
                Some(CleanupTerminator::Branch(edges)) => {
                    edges.capacity() * std::mem::size_of::<EdgeId>()
                }
                Some(CleanupTerminator::Goto(_) | CleanupTerminator::Exit(_)) | None => 0,
            }
    });
    let region_payload = builder.regions.iter().fold(0usize, |bytes, region| {
        bytes
            + region.slots.capacity() * std::mem::size_of::<StorageId>()
            + region
                .slots
                .iter()
                .map(storage_id_owned_capacity)
                .sum::<usize>()
    });
    let exit_payload = builder.exits.iter().fold(0usize, |bytes, exit| {
        bytes
            + exit.leaves_regions.capacity() * std::mem::size_of::<CleanupRegionId>()
            + exit.finalize_in_order.capacity() * std::mem::size_of::<FinalizeAction>()
            + exit
                .finalize_in_order
                .iter()
                .map(|action| {
                    cleanup_place_owned_capacity(&action.source)
                        + action.lifecycle_id.as_str().len()
                })
                .sum::<usize>()
            + exit_continuation_owned_capacity(&exit.continuation)
    });
    let status_payload = builder.status_sources.iter().fold(0usize, |bytes, source| {
        bytes
            + match &source.producer {
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                } => normalized_cases.capacity() * std::mem::size_of::<StatusCase>(),
                StatusProducer::PropagatedCall { callee } => callee.as_str().len(),
                StatusProducer::ContractFalse { .. } | StatusProducer::CheckedAllocation { .. } => {
                    0
                }
            }
            + status_source_id_owned_capacity(&source.id)
    });
    let edge_payload = builder
        .edges
        .iter()
        .map(|edge| edge_condition_owned_capacity(&edge.condition))
        .sum::<usize>();
    block_payload
        + region_payload
        + exit_payload
        + status_payload
        + edge_payload
        + builder.byte_helpers.owned_capacity_bytes()
        + strings::derived_metadata_capacity(builder)
        + flow_state_owned_capacity(&builder.initial_state)
        + builder.pending_try_residuals.capacity() * std::mem::size_of::<PendingTryResidual>()
        + builder
            .pending_try_residuals
            .iter()
            .map(|pending| flow_state_owned_capacity(&pending.state))
            .sum::<usize>()
        + builder.slots.capacity() * std::mem::size_of::<CleanupSlot>()
        + builder
            .slots
            .iter()
            .map(cleanup_slot_owned_capacity)
            .sum::<usize>()
        + builder.storage_to_slot.len()
            * (std::mem::size_of::<(StorageId, CleanupSlotId)>()
                + std::mem::size_of::<BTreeMap<StorageId, CleanupSlotId>>())
        + builder
            .storage_to_slot
            .keys()
            .map(storage_id_owned_capacity)
            .sum::<usize>()
        + builder.inventory_storage.len()
            * (std::mem::size_of::<(InventoryStorageId, StorageId)>()
                + std::mem::size_of::<BTreeMap<InventoryStorageId, StorageId>>())
        + builder
            .inventory_storage
            .values()
            .map(storage_id_owned_capacity)
            .sum::<usize>()
        + builder.leaves.len()
            * (std::mem::size_of::<(LivenessFlagId, LeafMetadata)>()
                + std::mem::size_of::<BTreeMap<LivenessFlagId, LeafMetadata>>())
        + builder
            .leaves
            .values()
            .map(|leaf| cleanup_place_owned_capacity(&leaf.place) + leaf.lifecycle.as_str().len())
            .sum::<usize>()
        + builder.entry_state.live_owned_parameters.capacity() * std::mem::size_of::<CleanupPlace>()
        + builder
            .entry_state
            .live_owned_parameters
            .iter()
            .map(cleanup_place_owned_capacity)
            .sum::<usize>()
        + builder.entry_state.conditional_owned_parameters.capacity()
            * std::mem::size_of::<ConditionalVariantEntry>()
        + builder
            .entry_state
            .conditional_owned_parameters
            .iter()
            .map(|entry| {
                storage_id_owned_capacity(&entry.storage)
                    + entry.variant.as_str().len()
                    + entry.cases.capacity() * std::mem::size_of::<ConditionalVariantCase>()
                    + entry
                        .cases
                        .iter()
                        .map(|case| {
                            case.case.as_str().len()
                                + case.live_places.capacity() * std::mem::size_of::<CleanupPlace>()
                                + case
                                    .live_places
                                    .iter()
                                    .map(cleanup_place_owned_capacity)
                                    .sum::<usize>()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
}

#[cfg(test)]
fn flow_state_owned_capacity(state: &FlowState) -> usize {
    state.live_order.capacity() * std::mem::size_of::<LivenessFlagId>()
        + state
            .renewals
            .iter()
            .map(|(at, history)| {
                at.as_str().len()
                    + history.owned_capacity()
                    + std::mem::size_of::<(
                        ExpressionId,
                        crate::cleanup_plan::renewal_history::RenewalHistory,
                    )>()
            })
            .sum::<usize>()
}

#[cfg(test)]
fn eval_result_owned_capacity(result: &EvalResult) -> usize {
    flow_state_owned_capacity(&result.state).saturating_add(
        result
            .owned_source
            .as_ref()
            .map_or(0, cleanup_place_owned_capacity),
    )
}

#[cfg(test)]
fn resolved_type_owned_capacity(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Function { parameters, result } => {
            parameters.capacity() * std::mem::size_of::<ResolvedType>()
                + std::mem::size_of::<ResolvedType>()
                + parameters
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
                + resolved_type_owned_capacity(result)
        }
        ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::MutFunctionI64
        | ResolvedType::Never
        | ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::StringMap => 0,
        ResolvedType::String | ResolvedType::Bytes | ResolvedType::Str | ResolvedType::SliceU8 => 0,
        ResolvedType::TypeParameter { owner, .. } => owner.as_str().len(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            declaration.as_str().len()
                + arguments.capacity() * std::mem::size_of::<ResolvedType>()
                + arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
fn resolved_param_owned_capacity(param: &crate::hir::ResolvedParam) -> usize {
    param.id.as_str().len() + param.name.capacity() + resolved_type_owned_capacity(&param.ty)
}

mod strings {
    use super::*;
    #[cfg(test)]
    pub(super) fn derived_metadata_capacity(builder: &PlanBuilder<'_>) -> usize {
        let lexical = builder.lexical.as_ref().map_or(0, |state| {
            state.owned_capacity()
                + super::lexical_control::capacity::lexical_exit_payload_extra(builder)
        });
        lexical
            + builder.string_appends.len()
                * (std::mem::size_of::<(ExpressionId, ExpressionId)>()
                    + std::mem::size_of::<BTreeMap<ExpressionId, ExpressionId>>())
            + builder
                .string_appends
                .iter()
                .map(|(operand, call)| operand.as_str().len() + call.as_str().len())
                .sum::<usize>()
            + builder.string_condition_reads.len()
                * (std::mem::size_of::<ExpressionId>()
                    + std::mem::size_of::<BTreeSet<ExpressionId>>())
            + builder
                .string_condition_reads
                .iter()
                .map(|id| id.as_str().len())
                .sum::<usize>()
    }
}

pub(super) fn construction_feedback(staged: usize, attached: usize) {
    note_lower_capacity_high_water(lower_capacity_high_water() + staged + attached);
}

/// Complete original program-overlap aggregation, including its own staged census.
pub(super) fn record_construction(plans: &Vec<CleanupPlan>, attached: usize) {
    // A conservative upper bound on the overlap: the largest in-progress
    // builder (including the held index) plus every staged plan backing.
    // Count full physical capacities, rather than legacy receipt lengths.
    let staged = plans
        .iter()
        .try_fold(
            plans.capacity() * std::mem::size_of::<CleanupPlan>(),
            |bytes, plan| {
                bytes.checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(plan)?,
                )
            },
        )
        .expect("staged physical capacity census fits usize");
    note_lower_capacity_high_water(lower_capacity_high_water() + staged + attached);
}
