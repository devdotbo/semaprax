//! Borrowed call signatures and one precharged current-HIR lookup cursor.
use super::*;
use crate::cleanup_plan::CallArgumentTransfer;

pub(super) struct CallCursor<'a> {
    frames: Vec<Option<(&'a ResolvedExpr, usize)>>,
}
impl<'a> CallCursor<'a> {
    pub(super) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let capacity = 514;
        budget.reserve_array::<Option<(&ResolvedExpr, usize)>>(
            function,
            capacity,
            "structural call cursor",
        )?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| replay_error(function, "structural call cursor allocation refused"))?;
        if frames.capacity() != capacity {
            return Err(replay_error(
                function,
                "structural call cursor capacity differs from reservation",
            ));
        }
        frames.resize(capacity, None);
        Ok(Self { frames })
    }
    fn find(
        &mut self,
        function: &'a ResolvedFunction,
        id: &ExpressionId,
        budget: &mut ReplayBudget,
    ) -> Result<&'a ResolvedExpr, Diagnostic> {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            self.frames[0] = Some((root, 0));
            let mut depth = 1;
            while depth != 0 {
                budget.charge(function, 1, "structural call lookup")?;
                let (node, next) = self.frames[depth - 1].expect("occupied structural call cursor");
                if next == 0 && node.id == *id {
                    return Ok(node);
                }
                if let Some(child) = crate::hir::lexical_exit::child(node, next) {
                    if depth == self.frames.len() {
                        return Err(replay_error(
                            function,
                            "structural call lookup depth exceeds bound",
                        ));
                    }
                    self.frames[depth - 1] = Some((node, next + 1));
                    self.frames[depth] = Some((child, 0));
                    depth += 1;
                } else {
                    self.frames[depth - 1] = None;
                    depth -= 1;
                }
            }
        }
        Err(replay_error(function, "call commit expression is absent"))
    }
}

enum Signature<'a> {
    Byte(crate::byte_ops::ByteOp),
    String(crate::string_ops::StringOp),
    Str(crate::str_ops::StrOp),
    Function(&'a ResolvedFunction),
}
impl<'a> Signature<'a> {
    fn parameter(&self, index: usize) -> Option<(OwnershipMode, &ResolvedType)> {
        match self {
            Self::Byte(op) => op
                .param_types()
                .get(index)
                .map(|ty| (op.param_ownership(index), ty)),
            Self::String(op) => op
                .param_types()
                .get(index)
                .map(|ty| (op.param_ownership(index), ty)),
            Self::Str(op) => op.param_types().get(index).map(|ty| {
                (
                    if *ty == ResolvedType::Str {
                        OwnershipMode::Borrow
                    } else {
                        OwnershipMode::Value
                    },
                    ty,
                )
            }),
            Self::Function(target) => target.params.get(index).map(|p| (p.ownership, &p.ty)),
        }
    }
    fn accepts_type(&self, index: usize, ty: &ResolvedType) -> bool {
        match self {
            // ArrayAsSlice carries a catalog placeholder extent; authenticate
            // its actual fixed extent through the owning builtin classifier.
            Self::Byte(op) => op.accepts_resolved(index, ty),
            _ => self
                .parameter(index)
                .is_some_and(|(_, expected)| expected == ty),
        }
    }
    fn len(&self) -> usize {
        match self {
            Self::Byte(op) => op.param_types().len(),
            Self::String(op) => op.param_types().len(),
            Self::Str(op) => op.param_types().len(),
            Self::Function(target) => target.params.len(),
        }
    }
}
fn needs_drop(function: &ResolvedFunction, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    match ty {
        ResolvedType::Bytes
        | ResolvedType::String
        | ResolvedType::StringMap
        | ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair => Ok(true),
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::MutFunctionI64 => Ok(false),
        ResolvedType::Never
        | ResolvedType::Function { .. }
        | ResolvedType::TypeParameter { .. }
        | ResolvedType::Nominal { .. } => Err(replay_error(
            function,
            "selected call commit has an unsupported parameter type",
        )),
    }
}
fn signature<'a>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    fact: &CallFact,
    budget: &mut ReplayBudget,
) -> Result<Signature<'a>, Diagnostic> {
    if fact.instance.is_some() || !fact.type_arguments.is_empty() {
        return Err(replay_error(
            function,
            "selected call commit does not support generic calls",
        ));
    }
    if let Some(op) = crate::byte_ops::by_id(fact.callee.as_str()) {
        return Ok(Signature::Byte(op));
    }
    if let Some(op) = crate::string_ops::by_id(fact.callee.as_str()) {
        if op == crate::string_ops::StringOp::Concat {
            return Err(replay_error(
                function,
                "selected call commit does not support String replacement",
            ));
        }
        return Ok(Signature::String(op));
    }
    if let Some(op) = crate::str_ops::by_id(fact.callee.as_str()) {
        return Ok(Signature::Str(op));
    }
    budget.charge(
        function,
        program.functions().len(),
        "structural current callee lookup",
    )?;
    let target = program
        .resolve_call_target(&fact.callee, None)
        .ok_or_else(|| {
            replay_error(
                function,
                format!("call commit has unknown callee `{}`", fact.callee),
            )
        })?;
    if !target.effects.is_empty() || target.yields.is_some() {
        return Err(replay_error(
            function,
            "selected call commit does not support effectful callees",
        ));
    }
    Ok(Signature::Function(target))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn validate<'a>(
    program: &dyn ReplayContext,
    function: &'a ResolvedFunction,
    call: &ExpressionId,
    fact: &CallFact,
    arguments: &[CallArgumentTransfer],
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    cursor: &mut CallCursor<'a>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let expression = cursor.find(function, call, budget)?;
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &expression.kind
    else {
        return Err(replay_error(
            function,
            "call commit does not name a selected ordinary call",
        ));
    };
    if callee != &fact.callee
        || type_arguments != &fact.type_arguments
        || instance != &fact.instance
        || args.len() != fact.arguments.len()
        || args
            .iter()
            .zip(&fact.arguments)
            .any(|(arg, id)| &arg.id != id)
    {
        return Err(replay_error(
            function,
            "call commit facts disagree with current typed call",
        ));
    }
    let signature = signature(program, function, fact, budget)?;
    if signature.len() != fact.arguments.len() {
        return Err(replay_error(
            function,
            "call commit callee signature has inconsistent arity",
        ));
    }
    let mut actual = arguments.iter();
    for index in 0..signature.len() {
        budget.charge(function, 1, "structural call parameters")?;
        let (mode, ty) = signature.parameter(index).expect("exact signature bound");
        let drop = needs_drop(function, ty)?;
        let arg = &args[index];
        if !signature.accepts_type(index, &arg.ty)
            || !match mode {
                OwnershipMode::Own => arg.ownership == OwnershipMode::Own,
                OwnershipMode::Borrow => {
                    matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
                }
                OwnershipMode::Value => arg.ownership == OwnershipMode::Value,
                OwnershipMode::Shared => false,
            }
        {
            return Err(replay_error(
                function,
                "call commit argument disagrees with exact parameter type or ownership",
            ));
        }
        if mode == OwnershipMode::Own && drop {
            let expected = u32::try_from(index)
                .map_err(|_| replay_error(function, "too many call parameters"))?;
            if actual
                .next()
                .is_none_or(|arg| arg.parameter_index != expected)
            {
                return Err(replay_error(
                    function,
                    "call commit does not contain every and only owned droppable parameter",
                ));
            }
        }
    }
    if actual.next().is_some() {
        return Err(replay_error(
            function,
            "call commit does not contain every and only owned droppable parameter",
        ));
    }
    let mut previous = None;
    let mut consumed = BTreeSet::new();
    for argument in arguments {
        budget.charge(function, 1, "structural call epochs")?;
        if previous.is_some_and(|value| value >= argument.parameter_index) {
            return Err(replay_error(
                function,
                "call-commit parameters are not in strict signature order",
            ));
        }
        previous = Some(argument.parameter_index);
        let index = usize::try_from(argument.parameter_index).map_err(|_| {
            replay_error(function, "call-commit parameter index does not fit usize")
        })?;
        let expected = fact.arguments.get(index).ok_or_else(|| {
            replay_error(function, "call-commit parameter index exceeds call arity")
        })?;
        if !matches!(&argument.source.storage,StorageId::CallArgument{call:storage_call,parameter_index,value_expression} if storage_call==call && parameter_index==&argument.parameter_index && value_expression==expected && argument.source.projections.is_empty())
        {
            return Err(replay_error(
                function,
                "call commit does not consume its matching whole argument epoch",
            ));
        }
        for flag in
            path_join::storage::validate_place(function, &argument.source, storage, leaves, budget)?
        {
            if consumed.contains(&flag) {
                return Err(replay_error(
                    function,
                    "atomic call commit consumes overlapping argument epochs",
                ));
            }
            budget.reserve_btree_node::<LivenessFlagId, ()>(
                function,
                "structural committed epoch flags",
            )?;
            consumed.insert(flag);
        }
    }
    Ok(())
}
