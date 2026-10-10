//! Exact preclone backing for the selected expression-fact owner.
use super::*;
use crate::cleanup_plan::replay::physical_storage::{btree_node_upper, PhysicalReservation};
use std::mem::size_of;

type TypeCursor<'a> = [Option<(&'a ResolvedType, usize)>; 514];
pub(super) struct FactCapacity<'a, 'p> {
    function: &'a ResolvedFunction,
    physical: &'p mut PhysicalReservation,
    types: TypeCursor<'a>,
    work: usize,
}
impl<'a, 'p> FactCapacity<'a, 'p> {
    pub(super) fn new(
        function: &'a ResolvedFunction,
        physical: &'p mut PhysicalReservation,
    ) -> Result<Self, Diagnostic> {
        // One reused type cursor and the collector's one active expression cursor.
        physical
            .reserve(size_of::<Self>() + size_of::<[Option<(&ResolvedExpr, usize)>; 514]>())
            .map_err(|_| {
                replay_error(
                    function,
                    "physical expression-fact cursor reservation refused",
                )
            })?;
        Ok(Self {
            function,
            physical,
            types: [None; 514],
            work: MAX_REPLAY_WORK_UNITS,
        })
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.physical.reserve(bytes).map_err(|_| {
            replay_error(
                self.function,
                "physical expression-fact reservation refused",
            )
        })
    }
    fn array<T>(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.physical.reserve_array::<T>(count).map_err(|_| {
            replay_error(
                self.function,
                "physical expression-fact array reservation refused",
            )
        })
    }
    fn types(&mut self, root: &'a ResolvedType) -> Result<(), Diagnostic> {
        self.types[0] = Some((root, 0));
        let mut len = 1;
        while len != 0 {
            self.work = self.work.checked_sub(1).ok_or_else(|| {
                replay_error(self.function, "expression-fact type work exceeds limit")
            })?;
            let (ty, next) = self.types[len - 1].expect("type cursor frame");
            if next == 0 {
                match ty {
                    ResolvedType::Function { parameters, .. } => {
                        self.array::<ResolvedType>(parameters.len())?;
                        self.reserve(size_of::<ResolvedType>())?;
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } => {
                        self.reserve(declaration.as_str().len())?;
                        self.array::<ResolvedType>(arguments.len())?;
                    }
                    ResolvedType::TypeParameter { owner, .. } => {
                        self.reserve(owner.as_str().len())?
                    }
                    ResolvedType::Never
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64
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
                    | ResolvedType::String
                    | ResolvedType::Bytes
                    | ResolvedType::Str
                    | ResolvedType::SliceU8
                    | ResolvedType::StringMap => {}
                }
            }
            let child = match ty {
                ResolvedType::Function { parameters, result } => {
                    if next == 0 {
                        Some(result.as_ref())
                    } else {
                        parameters.get(next - 1)
                    }
                }
                ResolvedType::Nominal { arguments, .. } => arguments.get(next),
                _ => None,
            };
            if let Some(child) = child {
                if len == self.types.len() {
                    return Err(replay_error(
                        self.function,
                        "expression-fact type depth exceeds limit",
                    ));
                }
                self.types[len - 1] = Some((ty, next + 1));
                self.types[len] = Some((child, 0));
                len += 1;
            } else {
                self.types[len - 1] = None;
                len -= 1;
            }
        }
        Ok(())
    }
    pub(super) fn expression(&mut self, expression: &'a ResolvedExpr) -> Result<(), Diagnostic> {
        self.work = self.work.checked_sub(1).ok_or_else(|| {
            replay_error(
                self.function,
                "expression-fact inventory exceeds work limit",
            )
        })?;
        self.reserve(
            btree_node_upper::<ExpressionId, Option<CallFact>>().ok_or_else(|| {
                replay_error(self.function, "expression-fact tree layout overflow")
            })?,
        )?;
        // ExpressionId clones retain existing Arc backing; only the new map and
        // argument slots allocate. Declaration/instance strings and types clone.
        if let Some((callee, args)) = crate::hir::closure::once::call(expression) {
            self.reserve(callee.as_str().len())?;
            return self.array::<ExpressionId>(args.len());
        }
        match &expression.kind {
            ResolvedExprKind::Invoke { callable, args } => {
                self.reserve("core.function.invoke".len())?;
                self.array::<ExpressionId>(args.len())?;
                self.array::<ResolvedType>(1)?;
                self.types(&callable.ty)?;
            }
            ResolvedExprKind::VecFieldRead { args, .. } => {
                self.reserve(crate::vec_field::ID.len())?;
                self.array::<ExpressionId>(args.len())?;
            }
            ResolvedExprKind::LiteralFormat { args, .. } => {
                self.reserve(crate::literal_format::ID.len())?;
                self.array::<ExpressionId>(args.len())?;
            }
            ResolvedExprKind::Call {
                callee,
                instance,
                args,
                type_arguments,
            } => {
                self.reserve(callee.as_str().len())?;
                if let Some(instance) = instance {
                    self.reserve(instance.as_str().len())?;
                }
                self.array::<ExpressionId>(args.len())?;
                self.array::<ResolvedType>(type_arguments.len())?;
                for ty in type_arguments {
                    self.types(ty)?;
                }
            }
            ResolvedExprKind::NativeRustImportCall(call)
                if crate::cleanup_plan::native_rust::owns(expression) =>
            {
                self.reserve(call.import.as_str().len())?;
                self.array::<ExpressionId>(call.args.len())?;
            }
            ResolvedExprKind::HostCommandCall(call) => {
                self.reserve(crate::command_io_ops::id(call.operation).len())?;
                self.array::<ExpressionId>(call.args.len())?;
            }
            ResolvedExprKind::ByteRange { operation, .. } => {
                self.reserve(operation.as_str().len())?;
                self.array::<ExpressionId>(3)?;
            }
            _ => {}
        }
        Ok(())
    }
}
