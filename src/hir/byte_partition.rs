//! Current-source authority for the private byte-partition profile.
//!
//! Registration alone is not admission. Ordinary resolution and validation
//! refuse the operation, and ordinary backends retain their refusal arms.
//! No caller can mint this owner from HIR, a descriptor, or module bytes.
use super::*;

/// The original immutable AST and the HIR resolved from that same AST.
/// The resolved subject stays private and is never replaced by a clone.
pub(crate) struct ValidatedPartitionProgram<'source> {
    source: &'source crate::ast::Program,
    program: ResolvedProgram,
}
impl<'source> ValidatedPartitionProgram<'source> {
    pub(crate) fn prepare(
        source: &'source crate::ast::Program,
        work: &mut usize,
    ) -> Result<Self, Diagnostic> {
        source_shape(source)?;
        let diagnostics = crate::source_verify::verify_partition(source);
        if let Some(diagnostic) = diagnostics.into_iter().find(|diagnostic| diagnostic.severity.is_error()) {
            return Err(diagnostic);
        }
        let (program, _) = Resolver {
            program: source,
            byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(source),
            declarations: DeclarationIndex::from_verified(source)?,
            reuse: None,
            function_work: FunctionResolutionWork::default(),
            fused_producer: None,
            partition_profile: true,
        }.resolve()?;
        let owner = Self { source, program };
        owner.revalidate(work)?;
        Ok(owner)
    }
    pub(crate) fn program(&self) -> &ResolvedProgram { &self.program }
    pub(crate) fn source(&self) -> &'source crate::ast::Program { self.source }
    /// The caller supplies the held invocation's remaining proof work. No
    /// receiver gets a fresh allowance by acquiring or reusing this owner.
    pub(crate) fn revalidate(&self, work: &mut usize) -> Result<(), Diagnostic> {
        let result = (|| {
            source_shape(self.source)?;
            // Full canonical status, cleanup, loan and identity checks remain
            // mandatory. Only the explicit ordinary-profile exclusion differs.
            validate_private(&self.program)?;
            scope(&self.program, work)
        })();
        if result.is_err() { *work = 0; }
        result
    }
}

impl Resolver<'_> {
    /// Both iterative and reference resolution use this identical guard.
    pub(super) fn byte_operation(&self, name: &str) -> Result<Option<crate::byte_ops::ByteOp>, Diagnostic> {
        let operation = crate::byte_ops::by_name(name);
        if operation == Some(crate::byte_ops::ByteOp::Partition) && !self.partition_profile {
            return Err(hir_error("byte partition requires the private checked source profile"));
        }
        Ok(operation)
    }
}

fn source_shape(source: &crate::ast::Program) -> Result<(), Diagnostic> {
    if !source.types.is_empty() || !source.module_uses.is_empty()
        || !source.interfaces.is_empty() || !source.protocols.is_empty()
        || !source.implementations.is_empty() || !source.session_protocols.is_empty()
        || !source.agents.is_empty() || !source.permits.is_empty()
        || source.functions.iter().any(|function| !function.type_parameters.is_empty()
            || !function.effects.is_empty() || function.yields.is_some() || function.follows.is_some()
            || function.stable_id == crate::construction_fusion::partition::ID
            || function.name == crate::construction_fusion::partition::NAME)
    {
        return Err(hir_error("private byte partition refuses declarations, imports, effects and generic functions"));
    }
    Ok(())
}

/// The same complete validator sequence, with no descriptor as a substitute.
pub(super) fn validate_private(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    agent_validation::validate(program)?;
    let selected = inspection::validate_with_static_text(program, None)?;
    crate::loan_plan::validate_program(program)?;
    Ok(selected)
}

pub(super) fn refuse_default(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in program.functions.iter().chain(program.function_instances.iter().map(|instance| &instance.function)) {
        for expression in function.requires.iter().chain(std::iter::once(&function.body)).chain(&function.ensures) {
            if contains_partition(expression)? { return Err(hir_error("byte partition requires the private checked source profile")); }
        }
    }
    for template in &program.function_templates {
        for expression in template.requires.iter().chain(std::iter::once(&template.body)).chain(&template.ensures) {
            if contains_partition(expression)? { return Err(hir_error("byte partition requires the private checked source profile")); }
        }
    }
    Ok(())
}

fn debit(work: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *work = work.checked_sub(amount).ok_or_else(|| hir_error("byte partition callgraph proof work exhausted"))?;
    Ok(())
}
fn admitted_type(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Never | ResolvedType::Unit | ResolvedType::I64 | ResolvedType::I32
        | ResolvedType::U8 | ResolvedType::Usize | ResolvedType::Bool | ResolvedType::Bytes
        | ResolvedType::SliceU8 | ResolvedType::ArrayU8(_) => true,
        ResolvedType::Nominal { declaration, arguments } => match (declaration.as_str(), arguments.as_slice()) {
            (crate::prelude::VEC_ID | crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID, [ResolvedType::Bytes]) => true,
            (crate::prelude::OPTION_ID, [ResolvedType::U8 | ResolvedType::Bytes]) => true,
            _ => false,
        },
        _ => false,
    }
}
fn scope(program: &ResolvedProgram, work: &mut usize) -> Result<(), Diagnostic> {
    if program.functions.len() > 512 { return Err(hir_error("private byte partition emitted callgraph exceeds 512 functions")); }
    let mut edges = [[0_u64; 8]; 512];
    if !program.function_instances.is_empty() || !program.function_templates.is_empty() {
        return Err(hir_error("private byte partition refuses generic instances"));
    }
    let mut found = false;
    for (ordinal, function) in program.functions.iter().enumerate() {
        debit(work, 1)?;
        if !admitted_type(&function.return_type) || function.params.iter().any(|p| !admitted_type(&p.ty)) {
            return Err(hir_error("private byte partition signature crosses an unsupported carrier"));
        }
        for root in function.requires.iter().chain(std::iter::once(&function.body)).chain(&function.ensures) {
            walk(root, work, &mut |expression, work| {
                if !admitted_type(&expression.ty) { return Err(hir_error("private byte partition expression crosses an unsupported carrier")); }
                match &expression.kind {
                    ResolvedExprKind::Call { callee, instance, type_arguments, .. } => {
                        debit(work, callee.as_str().len())?;
                        if instance.is_some() { return Err(hir_error("private byte partition refuses call instances")); }
                        if callee.as_str() == crate::construction_fusion::partition::ID {
                            crate::construction_fusion::partition::operation::typed(expression, work)
                                .map_err(|_| hir_error("private byte partition operation facts refused"))?
                                .ok_or_else(|| hir_error("private byte partition operation identity refused"))?;
                            found = true;
                        } else if crate::byte_ops::by_id(callee.as_str()).is_none()
                            && crate::vec_ops::by_id(callee.as_str()).is_none()
                            && crate::iterator_ops::by_id(callee.as_str()).is_none()
                        {
                            let mut current = false;
                            for (target_ordinal, target) in program.functions.iter().enumerate() {
                                if equal(target.id.as_str(), callee.as_str(), work)? {
                                    current = true;
                                    edges[ordinal][target_ordinal / 64] |= 1 << (target_ordinal % 64);
                                }
                            }
                            if !current || !type_arguments.is_empty() { return Err(hir_error("private byte partition refuses foreign calls")); }
                        }
                    }
                    ResolvedExprKind::Closure { .. } | ResolvedExprKind::Invoke { .. }
                    | ResolvedExprKind::FunctionReference { .. } => return Err(hir_error("private byte partition refuses callable carriers")),
                    _ => {}
                }
                Ok(())
            })?;
        }
    }
    if !found { return Err(hir_error("private byte partition has no current operation")); }
    let mut removed = [false; 512];
    for _ in 0..program.functions.len() {
        let mut candidate = None;
        for (ordinal, done) in removed[..program.functions.len()].iter().enumerate() {
            debit(work, 1)?;
            if *done { continue; }
            let mut outgoing = false;
            for (target, target_done) in removed[..program.functions.len()].iter().enumerate() {
                debit(work, 1)?;
                outgoing |= !target_done && edges[ordinal][target / 64] & (1 << (target % 64)) != 0;
            }
            if !outgoing { candidate = Some(ordinal); break; }
        }
        let ordinal = candidate.ok_or_else(|| hir_error("private byte partition refuses recursive callgraphs"))?;
        removed[ordinal] = true;
    }
    Ok(())
}
fn walk<'a>(root: &'a ResolvedExpr, work: &mut usize, visit: &mut impl FnMut(&'a ResolvedExpr, &mut usize) -> Result<(), Diagnostic>) -> Result<(), Diagnostic> {
    // Same accepted HIR depth as the lexical borrowed cursor; no heap scratch.
    let mut cursors = [None; 514];
    cursors[0] = Some(lexical_exit::ChildCursor::new(root));
    let mut depth = 1;
    while depth != 0 {
        let cursor = cursors[depth - 1].as_mut().expect("held cursor");
        if !cursor.is_started() { debit(work, 1)?; visit(cursor.expression(), work)?; }
        if let Some(child) = cursor.next() {
            if depth == cursors.len() { return Err(hir_error("private byte partition callgraph depth exhausted")); }
            cursors[depth] = Some(lexical_exit::ChildCursor::new(child));
            depth += 1;
        } else { cursors[depth - 1] = None; depth -= 1; }
    }
    Ok(())
}

fn equal(left: &str, right: &str, work: &mut usize) -> Result<bool, Diagnostic> {
    debit(work, 1)?;
    if left.len() != right.len() { return Ok(false); }
    for (a, b) in left.bytes().zip(right.bytes()) {
        debit(work, 1)?;
        if a != b { return Ok(false); }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;

// Ordinary admission needs no heap traversal allocation or recursive stack.
fn contains_partition(root: &ResolvedExpr) -> Result<bool, Diagnostic> {
    let mut cursors = [None; 514];
    cursors[0] = Some(lexical_exit::ChildCursor::new(root));
    let mut depth = 1;
    while depth != 0 {
        let cursor = cursors[depth - 1].as_mut().expect("held cursor");
        if !cursor.is_started() && matches!(&cursor.expression().kind, ResolvedExprKind::Call { callee, .. } if callee.as_str() == crate::construction_fusion::partition::ID) { return Ok(true); }
        if let Some(child) = cursor.next() {
            if depth == cursors.len() { return Err(hir_error("partition availability scan exceeds accepted HIR depth")); }
            cursors[depth] = Some(lexical_exit::ChildCursor::new(child));
            depth += 1;
        } else { cursors[depth - 1] = None; depth -= 1; }
    }
    Ok(false)
}
