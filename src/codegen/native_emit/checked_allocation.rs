//! Exact-source native receiving for the joined String allocation closure.
//! This private receiver never lends its selector to raw HIR or a decoded plan.
#![cfg_attr(not(test), allow(dead_code, reason = "checked native allocation receiver awaits its public application and SDK joins"))]
use super::*;
use crate::cleanup_plan::checked_allocation::Validated;
use crate::cleanup_plan::{AllocationOperation, StatusLane, StatusProducer};
mod constructors;
const KERNEL: &str = include_str!("checked_string_allocation/runtime.c");
const ADAPTER: &str = include_str!("checked_string_allocation/status_adapter.c");
const RECEIVER: &str = include_str!("checked_allocation/runtime.c");

pub(crate) struct Projection<'owner, 'source> { owner: &'owner Validated<'source> }
impl<'owner, 'source> Projection<'owner, 'source> {
    pub(crate) fn receive(owner: &'owner Validated<'source>) -> Result<Self, Diagnostic> {
        let projection = Self { owner };
        projection.revalidate()?;
        Ok(projection)
    }
    pub(super) fn owner(&self) -> &'owner Validated<'source> { self.owner }
    pub(super) fn revalidate(&self) -> Result<(), Diagnostic> {
        self.owner.replay()?;
        let program = self.owner.program();
        for function in program.functions.iter().chain(
            program.function_instances.iter().map(|instance| &instance.function),
        ) {
            if !function.effects.is_empty() || !leaf_type(&function.return_type)
                || function.params.iter().any(|parameter| !leaf_type(&parameter.ty)) {
                return Err(backend_error("checked native allocation receiver requires its closed String/scalar signature"));
            }
            for source in &function.cleanup_plan.status_sources {
                if let StatusProducer::CheckedAllocation { operation, normalized_cases } = &source.producer {
                    use AllocationOperation as A;
                    if source.id.lane != StatusLane::AllocationFailure
                        || *normalized_cases != [crate::cleanup_plan::AllocationCase::PayloadRefused,
                            crate::cleanup_plan::AllocationCase::SizeOverflow]
                        || !matches!(operation, A::StringLiteral | A::StringClone | A::StringConcat
                            | A::StringFromChar | A::StringFromI64 | A::StringFromUsize
                            | A::StringFromStr | A::StringFromUtf8 | A::StringSlice | A::StringTrim) {
                        return Err(backend_error("checked native allocation receiver has an unjoined materialization"));
                    }
                }
            }
            for root in function.requires.iter().chain(std::iter::once(&function.body)).chain(function.ensures.iter()) {
                classify(program, root)?;
                let mut stack = [const { None }; 514];
                stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
                let mut len = 1usize;
                while len != 0 {
                    if let Some(child) = stack[len - 1].as_mut().expect("retained receiver frame").next() {
                        classify(program, child)?;
                        if len == stack.len() { return Err(backend_error("checked native allocation receiver depth exceeds 512")); }
                        stack[len] = Some(hir::lexical_exit::ChildCursor::new(child));
                        len += 1;
                    } else { stack[len - 1] = None; len -= 1; }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn emit(&self) -> Result<String, Diagnostic> {
        super::emission::emit_checked_allocation_source(self)
    }
    pub(super) fn emit_entry(
        &self, output: &mut impl COutput,
        functions: &HashMap<FunctionExecutionId, CFunction>,
        resource_abi: &native_resource::NativeResourceAbi,
    ) -> Result<(), Diagnostic> {
        let execution = FunctionExecutionId::Monomorphic(self.owner.entry().clone());
        let metadata = functions.get(&execution).ok_or_else(|| backend_error("checked native entry is not held"))?;
        let function = self.owner.program().functions.iter().find(|function| function.id == *self.owner.entry())
            .ok_or_else(|| backend_error("checked native entry is not current"))?;
        let result = c_value_type(self.owner.program(), resource_abi, &function.return_type)?;
        write!(output, "\nstatic __attribute__((unused)) spx_status_token spx_checked_allocation_invoke_v1(struct spx_context *ctx, void *allocator_state, void *(*allocate)(void *, size_t), void (*release)(void *, void *), {result} *out) {{\n    struct spx_checked_string_adapter_v1 adapter = {{0}};\n    if (ctx == NULL || out == NULL || ctx->checked_allocation != NULL || ctx->status_arena.length != UINT32_C(0) || !spx_checked_string_adapter_init_v1(&adapter, ctx, allocator_state, allocate, release)) spx_runtime_invariant_failure(\"checked allocation invocation binding\");\n    ctx->checked_allocation = &adapter;\n    spx_status_token selected = {}(ctx, out);\n    ctx->checked_allocation = NULL;\n    return selected;\n}}\n", metadata.symbol).expect("String output");
        Ok(())
    }
}
fn leaf_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Unit | ResolvedType::I64 | ResolvedType::I32 | ResolvedType::U8
        | ResolvedType::Usize | ResolvedType::Char | ResolvedType::F32 | ResolvedType::F64
        | ResolvedType::Bool | ResolvedType::String | ResolvedType::Str | ResolvedType::SliceU8)
}
fn classify(program: &ResolvedProgram, expression: &ResolvedExpr) -> Result<(), Diagnostic> {
    if !leaf_type(&expression.ty) { return Err(backend_error("checked native allocation expression has an unjoined carrier")); }
    match &expression.kind {
        ResolvedExprKind::FunctionReference { .. } | ResolvedExprKind::HostCommandCall(_)
        | ResolvedExprKind::NativeRustImportCall(_) | ResolvedExprKind::LiteralFormat { .. } =>
            Err(backend_error("checked native allocation expression has an unjoined runtime helper")),
        ResolvedExprKind::Call { callee, instance, .. } => {
            use crate::string_ops::StringOp as S;
            if let Some(operation) = crate::string_ops::by_id(callee.as_str()) {
                if instance.is_some() || !matches!(operation, S::Take | S::Len | S::IsEmpty
                    | S::Concat | S::StartsWith | S::Contains | S::LenChars | S::FromChar
                    | S::FromI64 | S::FromUsize | S::FromStr | S::FromUtf8 | S::Slice | S::Trim
                    | S::Find | S::ByteAt | S::Compare | S::F64FromI64 | S::I64FromF64
                    | S::UsizeFromI64 | S::I64FromU8 | S::I64FromI32 | S::UsizeFromU8
                    | S::U8FromI64 | S::CharFromU8 | S::CharFromI64 | S::I64FromUsize) {
                    return Err(backend_error("checked native allocation call has an unjoined intrinsic"));
                }
                Ok(())
            } else if match instance {
                Some(id) => program.function_instances.iter().any(|current| current.id == *id),
                None => program.functions.iter().any(|current| current.id == *callee),
            } { Ok(()) } else { Err(backend_error("checked native allocation call lacks a current held callee")) }
        }
        _ => Ok(()), // Full HIR and coverage were independently checked first.
    }
}

fn join_runtime(prelude: String) -> Result<String, Diagnostic> {
    let header = "struct spx_string_v10 { uint64_t len; char data[]; };";
    let context = "    uint32_t call_depth;";
    let zero = "context->call_depth == UINT32_C(0);";
    if prelude.matches(header).count() != 1 || prelude.matches(context).count() != 1
        || prelude.matches(zero).count() != 1 {
        return Err(backend_error("checked native runtime layout anchor is not unique"));
    }
    let mut output = prelude.replacen(header,
        "struct spx_string_v10 { void *release_state; void (*release)(void *, void *); uint64_t len; char data[]; };", 1)
        .replacen(context, "    uint32_t call_depth;\n    struct spx_checked_string_adapter_v1 *checked_allocation;", 1)
        .replacen(zero, "context->call_depth == UINT32_C(0) && context->checked_allocation == NULL;", 1);
    // Reuse the independently checked borrowed constructor algorithms with the
    // SAME concrete carrier used by all ordinary read helpers in this unit.
    let kernel = KERNEL.replace("struct spx_checked_string_header_v1 { uint64_t length; char data[]; };", "")
        .replace("spx_checked_string_header_v1", "spx_string_v10")
        .replace("->length", "->len")
        .replace("static ", "static __attribute__((unused)) ")
        .replace("ctx->release(ctx->allocator_state, spx_string_v10(text));",
            "struct spx_string_v10 *header = spx_string_v10(text); (void)ctx; header->release(header->release_state, header);")
        .replace("value->len = length;", "value->release_state = ctx->allocator_state; value->release = ctx->release; value->len = length;");
    output.push_str("\n#define SPX_CHECKED_STRING_KERNEL_ONLY_V1 1\n");
    output.push_str(&kernel);
    output.push_str(ADAPTER);
    output.push_str(RECEIVER);
    Ok(output)
}
#[cfg(test)]
#[path = "checked_allocation/tests.rs"]
mod tests;
