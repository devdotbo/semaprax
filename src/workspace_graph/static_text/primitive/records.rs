//! Private source-captured flat-record facts, held before the AST is discarded.
//! These facts are checked layout claims, never a replay admission switch.
use super::*;
use crate::ast::TypeDeclarationKind;
const MAX_RECORDS: usize = 32;
const MAX_FIELDS: usize = 8;
pub(in crate::workspace_graph) struct Records {
    pub(in crate::workspace_graph) records: Vec<RecordFact>,
}
pub(in crate::workspace_graph) struct RecordFact {
    pub(in crate::workspace_graph) declaration: hir::ResolvedTypeDeclaration,
    pub(in crate::workspace_graph) explicit: bool,
    pub(in crate::workspace_graph) field_explicit: Vec<bool>,
}
fn field_type(ty: &Type) -> Option<hir::ResolvedType> {
    let current = super::ty(ty)?;
    (hir::is_scalar_resolved_type(&current)
        || matches!(
            current,
            hir::ResolvedType::String | hir::ResolvedType::Bytes
        ))
    .then_some(current)
}
fn debit(work: &mut usize, n: usize) -> Result<(), Vec<Diagnostic>> {
    *work = work.checked_sub(n).ok_or_else(fail)?;
    Ok(())
}
fn equal(work: &mut usize, a: &str, b: &str) -> Result<bool, Vec<Diagnostic>> {
    debit(work, 1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        debit(work, 1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(in crate::workspace_graph) fn preflight(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    if program.types.len() > MAX_RECORDS {
        return Ok(false);
    }
    for (position, source) in program.types.iter().enumerate() {
        debit(work, position.checked_add(1).ok_or_else(fail)?)?;
        if !source.type_parameters.is_empty()
            || source.extends.is_some()
            || !source.invariants().is_empty()
        {
            return Ok(false);
        }
        for old in &program.types[..position] {
            if equal(work, &old.stable_id, &source.stable_id)?
                || equal(work, &old.name, &source.name)?
            {
                return Ok(false);
            }
        }
        let TypeDeclarationKind::Record { fields } = &source.kind else {
            return Ok(false);
        };
        if !(1..=MAX_FIELDS).contains(&fields.len()) {
            return Ok(false);
        }
        debit(work, resolved.types.len())?;
        let mut selected = None;
        for record in &resolved.types {
            if equal(work, record.id.as_str(), &source.stable_id)?
                && selected.replace(record).is_some()
            {
                return Err(fail());
            }
        }
        let current = selected.ok_or_else(fail)?;
        if !equal(work, &current.name, &source.name)?
            || current.span != source.span
            || !current.type_parameters.is_empty()
        {
            return Err(fail());
        }
        let hir::ResolvedTypeDeclarationKind::Record {
            fields: current_fields,
        } = &current.kind
        else {
            return Err(fail());
        };
        if fields.len() != current_fields.len() {
            return Err(fail());
        }
        for (ordinal, (source, current)) in fields.iter().zip(current_fields).enumerate() {
            debit(work, ordinal.checked_add(1).ok_or_else(fail)?)?;
            let Some(ty) = field_type(&source.ty) else {
                return Ok(false);
            };
            if !equal(work, &source.stable_id, current.id.as_str())?
                || !equal(work, &source.name, &current.name)?
                || source.span != current.span
                || current.index as usize != ordinal
                || current.ty != ty
            {
                return Err(fail());
            }
            for old in &fields[..ordinal] {
                if equal(work, &old.stable_id, &source.stable_id)?
                    || equal(work, &old.name, &source.name)?
                {
                    return Err(fail());
                }
            }
        }
    }
    Ok(true)
}
pub(in crate::workspace_graph) fn source_type_matches(
    program: &Program,
    source: &Type,
    current: &hir::ResolvedType,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    if let Type::Named { name, arguments } = source {
        let hir::ResolvedType::Nominal {
            declaration,
            arguments: actual,
        } = current
        else {
            return Ok(false);
        };
        if equal(work, name, "Vec")? {
            let ([element], [current]) = (arguments.as_slice(), actual.as_slice()) else {
                return Ok(false);
            };
            return Ok(equal(work, declaration.as_str(), crate::prelude::VEC_ID)?
                && vec_element_admitted(program, element, work)?
                && source_type_matches(program, element, current, work)?);
        }
        if !arguments.is_empty() || !actual.is_empty() {
            return Ok(false);
        }
        let mut count = 0usize;
        for record in &program.types {
            if equal(work, &record.name, name)?
                && equal(work, &record.stable_id, declaration.as_str())?
            {
                count = count.checked_add(1).ok_or_else(fail)?;
            }
        }
        for import in &program.module_uses {
            debit(work, 1)?;
            if import.kind == crate::ast::ModuleUseKind::Type
                && equal(work, &import.alias, name)?
                && equal(work, &import.persistent_id, declaration.as_str())?
            {
                count = count.checked_add(1).ok_or_else(fail)?;
            }
        }
        Ok(count == 1)
    } else {
        debit(work, 1)?;
        Ok(super::ty(source).as_ref() == Some(current))
    }
}
pub(in crate::workspace_graph) fn source_type_admitted(
    program: &Program,
    source: &Type,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    if let Type::Named { name, arguments } = source {
        if equal(work, name, "Vec")? {
            let [element] = arguments.as_slice() else {
                return Ok(false);
            };
            return vec_element_admitted(program, element, work);
        }
        if !arguments.is_empty() {
            return Ok(false);
        }
        let mut count = 0usize;
        for record in &program.types {
            if equal(work, &record.name, name)? {
                count = count.checked_add(1).ok_or_else(fail)?;
            }
        }
        for import in &program.module_uses {
            debit(work, 1)?;
            if import.kind == crate::ast::ModuleUseKind::Type && equal(work, &import.alias, name)? {
                count = count.checked_add(1).ok_or_else(fail)?;
            }
        }
        Ok(count == 1)
    } else {
        debit(work, 1)?;
        Ok(super::ty(source).is_some())
    }
}
fn vec_element_admitted(
    program: &Program,
    element: &Type,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    debit(work, 1)?;
    if crate::vec_ops::ast_element_is_admitted(element)
        || matches!(element, Type::String | Type::Bytes)
    {
        return Ok(true);
    }
    if matches!(element, Type::Named { arguments, .. } if arguments.is_empty()) {
        source_type_admitted(program, element, work)
    } else {
        Ok(false)
    }
}
pub(super) fn clone_type(
    current: &hir::ResolvedType,
) -> Result<hir::ResolvedType, Vec<Diagnostic>> {
    if let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = current
    {
        reserve_builder_structure(declaration.as_str().len())?;
        if declaration.as_str() == crate::prelude::VEC_ID {
            let [element] = arguments.as_slice() else {
                return Err(fail());
            };
            if matches!(element, hir::ResolvedType::Nominal { arguments, .. } if !arguments.is_empty())
            {
                return Err(fail());
            }
            reserve_builder_structure(std::mem::size_of::<hir::ResolvedType>())?;
            let mut cloned = Vec::with_capacity(1);
            if cloned.capacity() != 1 {
                return Err(fail());
            }
            cloned.push(clone_type(element)?);
            return Ok(hir::ResolvedType::Nominal {
                declaration: declaration.clone(),
                arguments: cloned,
            });
        }
        if !arguments.is_empty() {
            return Err(fail());
        }
    }
    Ok(current.clone())
}
pub(super) fn type_capacity_bytes(current: &hir::ResolvedType) -> Option<usize> {
    if let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = current
    {
        arguments.iter().try_fold(
            declaration.as_str().len().checked_add(
                arguments
                    .capacity()
                    .checked_mul(std::mem::size_of::<hir::ResolvedType>())?,
            )?,
            |n, child| n.checked_add(type_capacity_bytes(child)?),
        )
    } else {
        Some(0)
    }
}
pub(in crate::workspace_graph) fn capture(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    work: &mut usize,
) -> Result<Option<Box<Records>>, Vec<Diagnostic>> {
    if program.types.is_empty() {
        return Ok(None);
    }
    if !preflight(program, resolved, work)? {
        return Err(fail());
    }
    reserve_builder_structure(
        std::mem::size_of::<Records>()
            .checked_add(
                program
                    .types
                    .len()
                    .checked_mul(std::mem::size_of::<RecordFact>())
                    .ok_or_else(capacity)?,
            )
            .ok_or_else(capacity)?,
    )?;
    let mut records = Vec::with_capacity(program.types.len());
    for source in &program.types {
        *work = work.checked_sub(resolved.types.len()).ok_or_else(fail)?;
        let mut selected = None;
        for record in &resolved.types {
            if equal(work, record.id.as_str(), &source.stable_id)? {
                selected = Some(record);
                break;
            }
        }
        let current = selected.ok_or_else(fail)?;
        let hir::ResolvedTypeDeclarationKind::Record { fields } = &current.kind else {
            return Err(fail());
        };
        let TypeDeclarationKind::Record {
            fields: source_fields,
        } = &source.kind
        else {
            return Err(fail());
        };
        let field_bytes = fields.iter().try_fold(
            fields
                .len()
                .checked_mul(std::mem::size_of::<hir::ResolvedFieldDeclaration>())
                .ok_or_else(capacity)?,
            |sum, field| {
                sum.checked_add(field.id.as_str().len())
                    .and_then(|n| n.checked_add(field.name.len()))
                    .ok_or_else(capacity)
            },
        )?;
        reserve_builder_structure(
            current
                .id
                .as_str()
                .len()
                .checked_add(current.name.len())
                .and_then(|n| n.checked_add(field_bytes))
                .and_then(|n| n.checked_add(fields.len()))
                .ok_or_else(capacity)?,
        )?;
        let field_explicit = source_fields
            .iter()
            .map(|field| field.explicit_id)
            .collect::<Vec<_>>();
        records.push(RecordFact {
            declaration: current.clone(),
            explicit: source.explicit_id,
            field_explicit,
        });
    }
    if crate::bounded_output::active_refused() {
        return Err(capacity());
    }
    Ok(Some(Box::new(Records { records })))
}
impl Records {
    pub(in crate::workspace_graph) fn owned_capacity_bytes(&self) -> Option<usize> {
        self.records.iter().try_fold(
            std::mem::size_of::<Records>().checked_add(
                self.records
                    .capacity()
                    .checked_mul(std::mem::size_of::<RecordFact>())?,
            )?,
            |sum, record| {
                let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.declaration.kind
                else {
                    return None;
                };
                let fields = fields.iter().try_fold(
                    fields
                        .capacity()
                        .checked_mul(std::mem::size_of::<hir::ResolvedFieldDeclaration>())?,
                    |sum, field| {
                        sum.checked_add(field.id.as_str().len())?
                            .checked_add(field.name.capacity())
                    },
                )?;
                sum.checked_add(record.declaration.id.as_str().len())?
                    .checked_add(record.declaration.name.capacity())?
                    .checked_add(fields)?
                    .checked_add(record.field_explicit.capacity())
            },
        )
    }
}
