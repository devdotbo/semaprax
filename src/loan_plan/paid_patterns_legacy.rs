//! Original pattern inventory order and last-wins replacement, caller-paid.
use super::*;

pub(super) fn inventory(
    pattern: &ResolvedMatchPattern,
    ownership: &mut BTreeMap<ValueId, OwnershipMode>,
    payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
) -> Result<(), Diagnostic> {
    fn record_field(
        pattern: &ResolvedRecordMatchFieldPattern,
        ownership: &mut BTreeMap<ValueId, OwnershipMode>,
        payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
    ) -> Result<(), Diagnostic> {
        match pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                storage::map_insert(ownership, binding.id.clone(), binding.ownership, payer)?;
            }
            ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                for field in fields {
                    record_field(&field.pattern, ownership, payer)?;
                }
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
        }
        Ok(())
    }

    match pattern {
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                storage::map_insert(
                    ownership,
                    field.binding.id.clone(),
                    field.binding.ownership,
                    payer,
                )?;
            }
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            for field in fields {
                record_field(&field.pattern, ownership, payer)?;
            }
        }
        ResolvedMatchPattern::Binding(binding) => {
            storage::map_insert(ownership, binding.id.clone(), binding.ownership, payer)?;
        }
        ResolvedMatchPattern::Or(alternatives) => {
            for alternative in alternatives {
                inventory(alternative, ownership, payer)?;
            }
        }
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
    }
    Ok(())
}
