//! Authored field and loop identities for scoped record traversal. This replay
//! derives its layout from source declarations, never from retained HIR reads.
use super::{graph_error, reserve_builder_structure, WorkspaceResolvedModule};
use crate::ast::{
    Expr, ExprKind, Function, ModuleUseKind, Program, Statement, Type, TypeDeclaration,
    TypeDeclarationKind,
};
use crate::bounded_output::budgeted_format;
use crate::{diagnostic::Diagnostic, hir};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
fn refusal() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "record traversal source field, index, carrier or loop identity disagrees with retained HIR",
    )]
}

pub(super) struct Read<'a> {
    pub(super) function: &'a Function,
    pub(super) path: String,
    pub(super) declaration: &'a TypeDeclaration,
    pub(super) field: &'a crate::ast::FieldDeclaration,
    pub(super) root: hir::ValueId,
    pub(super) index: hir::ValueId,
    pub(super) copy: bool,
    pub(super) bytes: bool,
}
pub(super) struct Loop<'a> {
    pub(super) function: &'a Function,
    pub(super) path: String,
    pub(super) declaration: &'a TypeDeclaration,
    root: hir::ValueId,
    index: hir::ValueId,
    index_name: &'a str,
}
pub(super) struct Expected<'a> {
    pub(super) reads: Vec<Read<'a>>,
    pub(super) loops: Vec<Loop<'a>>,
}
#[derive(Clone)]
struct Binding<'a> {
    name: &'a str,
    element: Option<(&'a str, &'a TypeDeclaration)>,
    id: hir::ValueId,
}
struct Row<'a> {
    name: &'a str,
    declaration: &'a TypeDeclaration,
    root: hir::ValueId,
    index: hir::ValueId,
}

fn declaration<'a>(
    program: &'a Program,
    programs: &'a [Program],
    name: &'a str,
) -> Option<&'a TypeDeclaration> {
    program.types.iter().find(|ty| ty.name == name).or_else(|| {
        let import = program
            .module_uses
            .iter()
            .find(|item| item.kind == ModuleUseKind::Type && item.alias == name)?;
        programs
            .iter()
            .find(|candidate| candidate.module == import.target_module)?
            .types
            .iter()
            .find(|ty| ty.stable_id == import.persistent_id)
    })
}
fn vec_element<'a>(
    program: &'a Program,
    programs: &'a [Program],
    ty: &'a Type,
) -> Option<(&'a str, &'a TypeDeclaration)> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    let [Type::Named {
        name: element,
        arguments: empty,
    }] = arguments.as_slice()
    else {
        return None;
    };
    (name == "Vec" && empty.is_empty())
        .then(|| declaration(program, programs, element).map(|decl| (element.as_str(), decl)))
        .flatten()
}
fn inferred<'a>(
    program: &'a Program,
    programs: &'a [Program],
    expression: &'a Expr,
    bindings: &[Binding<'a>],
) -> Option<(&'a str, &'a TypeDeclaration)> {
    match &expression.kind {
        ExprKind::Var(name) => {
            bindings
                .iter()
                .rev()
                .find(|binding| binding.name == name)?
                .element
        }
        ExprKind::Call {
            name,
            type_arguments,
            ..
        } => {
            if let Some(operation) = crate::vec_ops::by_name(name) {
                let [Type::Named {
                    name: element,
                    arguments,
                }] = type_arguments.as_slice()
                else {
                    return None;
                };
                if !arguments.is_empty()
                    || !matches!(operation.ast_return_type(&type_arguments[0]), Type::Named { ref name, .. } if name == "Vec")
                {
                    return None;
                }
                return declaration(program, programs, element)
                    .map(|decl| (element.as_str(), decl));
            }
            let (context, function) = if let Some(function) = program
                .functions
                .iter()
                .find(|function| function.name == *name)
            {
                (program, function)
            } else {
                let import = program
                    .module_uses
                    .iter()
                    .find(|item| item.kind == ModuleUseKind::Function && item.alias == *name)?;
                let context = programs
                    .iter()
                    .find(|candidate| candidate.module == import.target_module)?;
                (
                    context,
                    context
                        .functions
                        .iter()
                        .find(|function| function.stable_id == import.persistent_id)?,
                )
            };
            if let Some(element) = vec_element(context, programs, &function.return_type) {
                return Some(element);
            }
            let Type::Named { name, arguments } = &function.return_type else {
                return None;
            };
            let [Type::Named {
                name: parameter,
                arguments: empty,
            }] = arguments.as_slice()
            else {
                return None;
            };
            if name != "Vec" || !empty.is_empty() {
                return None;
            }
            let position = function
                .type_parameters
                .iter()
                .position(|item| item.name == *parameter)?;
            let Type::Named {
                name: element,
                arguments,
            } = type_arguments.get(position)?
            else {
                return None;
            };
            if !arguments.is_empty() {
                return None;
            }
            declaration(program, programs, element).map(|decl| (element.as_str(), decl))
        }
        ExprKind::Block { tail, .. } => inferred(program, programs, tail, bindings),
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            let left = inferred(program, programs, then_branch, bindings)?;
            let right = inferred(program, programs, else_branch, bindings)?;
            (left.1.stable_id == right.1.stable_id).then_some(left)
        }
        _ => None,
    }
}
fn execution(function: &Function) -> hir::FunctionExecutionId {
    hir::FunctionExecutionId::Monomorphic(hir::DeclarationId::new(&function.stable_id))
}
fn has_traversal(expression: &Expr) -> bool {
    if matches!(&expression.kind, ExprKind::Block { statements, .. } if statements.iter().any(|statement| matches!(statement, Statement::ForBorrow { .. })))
    {
        return true;
    }
    let mut index = 0;
    while let Some(child) = expression.child(index) {
        if has_traversal(child) {
            return true;
        }
        index += 1;
    }
    false
}

fn reserve_replay(function: &Function) -> Result<()> {
    fn census(expression: &Expr, depth: usize, total: &mut usize) -> Option<()> {
        let bytes = depth.checked_mul(32)?.checked_add(512)?;
        *total = total.checked_add(bytes)?;
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            census(child, depth.checked_add(1)?, total)?;
            index += 1;
        }
        Some(())
    }
    let mut total = function.params.len().checked_mul(512).ok_or_else(refusal)?;
    census(&function.body, 1, &mut total).ok_or_else(refusal)?;
    let nodes = total / 512 + 1;
    let identities = nodes
        .checked_mul(
            function
                .stable_id
                .len()
                .checked_mul(8)
                .ok_or_else(refusal)?,
        )
        .ok_or_else(refusal)?;
    reserve_builder_structure(total.checked_add(identities).ok_or_else(refusal)?)
}
pub(super) fn collect<'a>(program: &'a Program, programs: &'a [Program]) -> Result<Expected<'a>> {
    let mut expected = Expected {
        reads: Vec::new(),
        loops: Vec::new(),
    };
    for function in &program.functions {
        if !has_traversal(&function.body) {
            continue;
        }
        if !function.type_parameters.is_empty() {
            return Err(refusal());
        }
        // Replay allocates source binding identities and a bounded HIR walk
        // frontier. Reserve their complete conservative physical envelope
        // before constructing the first helper carrier; legacy functions skip it.
        reserve_replay(function)?;
        let mut bindings = Vec::new();
        for (index, parameter) in function.params.iter().enumerate() {
            reserve_builder_structure(4 * std::mem::size_of::<Binding<'_>>())?;
            bindings.push(Binding {
                name: &parameter.name,
                element: vec_element(program, programs, &parameter.ty),
                id: hir::ValueId::parameter(&execution(function), index),
            });
        }
        walk(
            program,
            programs,
            function,
            &function.body,
            "body",
            &mut bindings,
            None,
            &mut expected,
        )?;
    }
    Ok(expected)
}

#[allow(clippy::too_many_arguments)]
fn walk<'a>(
    program: &'a Program,
    programs: &'a [Program],
    function: &'a Function,
    expression: &'a Expr,
    path: &str,
    bindings: &mut Vec<Binding<'a>>,
    row: Option<&Row<'a>>,
    out: &mut Expected<'a>,
) -> Result<()> {
    let selected = match &expression.kind {
        ExprKind::Project { base, field, .. } => row
            .filter(|row| matches!(&base.kind, ExprKind::Var(name) if name == row.name))
            .map(|row| (row, field.as_str(), false)),
        ExprKind::Call {
            name,
            type_arguments,
            args,
        } if name == "str_as_bytes" && type_arguments.is_empty() => match args.as_slice() {
            [Expr {
                kind: ExprKind::Project { base, field, .. },
                ..
            }] => row
                .filter(|row| matches!(&base.kind, ExprKind::Var(name) if name == row.name))
                .map(|row| (row, field.as_str(), true)),
            _ => None,
        },
        _ => None,
    };
    if let Some((row, name, bytes)) = selected {
        let TypeDeclarationKind::Record { fields } = &row.declaration.kind else {
            return Err(refusal());
        };
        let field = fields
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(refusal)?;
        let copy = fields
            .iter()
            .all(|field| !matches!(field.ty, Type::String | Type::Bytes));
        if bytes && field.ty != Type::String {
            return Err(refusal());
        }
        reserve_builder_structure(4 * std::mem::size_of::<Read<'_>>())?;
        out.reads.push(Read {
            function,
            path: crate::bounded_output::budgeted_clone(path),
            declaration: row.declaration,
            field,
            root: row.root.clone(),
            index: row.index.clone(),
            copy,
            bytes,
        });
        return Ok(());
    }
    macro_rules! child {
        ($expression:expr, $suffix:expr, $bindings:expr, $row:expr $(,)?) => {
            walk(
                program,
                programs,
                function,
                $expression,
                &budgeted_format(format_args!("{path}.{}", $suffix)),
                $bindings,
                $row,
                out,
            )
        };
    }
    match &expression.kind {
        ExprKind::Block { statements, tail } => {
            let base = bindings.len();
            for (position, statement) in statements.iter().enumerate() {
                match statement {
                    Statement::Let {
                        name,
                        declared,
                        value,
                        ..
                    } => {
                        child!(
                            value,
                            &budgeted_format(format_args!("s{position}.value")),
                            bindings,
                            row
                        )?;
                        let element = declared
                            .as_ref()
                            .and_then(|ty| vec_element(program, programs, ty))
                            .or_else(|| inferred(program, programs, value, bindings));
                        reserve_builder_structure(4 * std::mem::size_of::<Binding<'_>>())?;
                        bindings.push(Binding {
                            name,
                            element,
                            id: hir::ValueId::local(
                                &execution(function),
                                &budgeted_format(format_args!("{path}.s{position}")),
                            ),
                        });
                    }
                    Statement::ForBorrow {
                        item,
                        index,
                        values,
                        body,
                        ..
                    } => {
                        if row.is_some() {
                            return Err(refusal());
                        }
                        let ExprKind::Var(name) = &values.kind else {
                            return Err(refusal());
                        };
                        let binding = bindings
                            .iter()
                            .rev()
                            .find(|binding| binding.name == name)
                            .ok_or_else(refusal)?;
                        let (_alias, declaration) = binding.element.ok_or_else(refusal)?;
                        let region_path = budgeted_format(format_args!("{path}.s{position}.value"));
                        let index_id = hir::ValueId::local(
                            &execution(function),
                            &budgeted_format(format_args!("{region_path}.s2.body.s0")),
                        );
                        reserve_builder_structure(4 * std::mem::size_of::<Loop<'_>>())?;
                        let context = Row {
                            name: item,
                            declaration,
                            root: binding.id.clone(),
                            index: index_id.clone(),
                        };
                        out.loops.push(Loop {
                            function,
                            path: region_path,
                            declaration,
                            root: binding.id.clone(),
                            index: index_id,
                            index_name: index
                                .as_ref()
                                .map_or("#for-row-index", |index| index.name.as_str()),
                        });
                        child!(
                            body,
                            &budgeted_format(format_args!("s{position}.value.s2.body.s1.value")),
                            bindings,
                            Some(&context),
                        )?;
                    }
                    _ => {
                        for index in 0..statement.child_count() {
                            child!(
                                statement.child(index).expect("statement child"),
                                &budgeted_format(format_args!(
                                    "s{position}.{}",
                                    super::expected_projection::statement_segment::child(
                                        statement, index
                                    )
                                )),
                                bindings,
                                row,
                            )?;
                        }
                    }
                }
            }
            child!(tail, "tail", bindings, row)?;
            bindings.truncate(base);
        }
        ExprKind::Call { args, .. } => {
            for (index, argument) in args.iter().enumerate() {
                child!(
                    argument,
                    &budgeted_format(format_args!("arg.{index}")),
                    bindings,
                    row
                )?;
            }
        }
        ExprKind::VecConstructor { value, .. } | ExprKind::Unary { value, .. } => {
            child!(value, "value", bindings, row)?
        }
        ExprKind::Binary { left, right, .. } => {
            child!(left, "left", bindings, row)?;
            child!(right, "right", bindings, row)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            child!(condition, "condition", bindings, row)?;
            child!(then_branch, "then", bindings, row)?;
            child!(else_branch, "else", bindings, row)?;
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for (index, field) in fields.iter().enumerate() {
                child!(
                    &field.value,
                    &budgeted_format(format_args!("field.{index}.value")),
                    bindings,
                    row
                )?;
            }
        }
        ExprKind::Project { base, .. } => child!(base, "base", bindings, row)?,
        ExprKind::UpdateRecord { base, fields } => {
            child!(base, "base", bindings, row)?;
            for (index, field) in fields.iter().enumerate() {
                child!(
                    &field.value,
                    &budgeted_format(format_args!("field.{index}.value")),
                    bindings,
                    row
                )?;
            }
        }
        ExprKind::Try { operand } | ExprKind::Yield { request: operand } => {
            child!(operand, "operand", bindings, row)?
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            child!(scrutinee, "scrutinee", bindings, row)?;
            for (index, arm) in arms.iter().enumerate() {
                if let Some(guard) = &arm.guard {
                    child!(
                        guard,
                        &budgeted_format(format_args!("arm.{index}.guard")),
                        bindings,
                        row
                    )?;
                }
                child!(
                    &arm.value,
                    &budgeted_format(format_args!("arm.{index}.value")),
                    bindings,
                    row
                )?;
            }
        }
        ExprKind::Closure { body, .. } => {
            if row.is_some() {
                return Err(refusal());
            }
            child!(body, "closure.body", bindings, None)?;
        }
        ExprKind::MethodCall { .. } | ExprKind::SuperMethod { .. }
            if has_traversal(expression) || row.is_some() =>
        {
            return Err(refusal());
        }
        _ => {}
    }
    Ok(())
}

fn exact_place(expression: &hir::ResolvedExpr, expected: &hir::ValueId) -> bool {
    matches!(&expression.kind, hir::ResolvedExprKind::Place(place) if &place.root == expected && place.projections.is_empty())
}
fn element_matches(element: &hir::ResolvedType, declaration: &TypeDeclaration) -> bool {
    matches!(element, hir::ResolvedType::Nominal { declaration: id, arguments } if id.as_str() == declaration.stable_id && arguments.is_empty())
}

pub(super) fn validate(programs: &[Program], modules: &[WorkspaceResolvedModule]) -> Result<()> {
    for program in programs {
        let expected = collect(program, programs)?;
        if expected.loops.is_empty() {
            continue;
        }
        let module = modules
            .iter()
            .find(|module| module.module == program.module)
            .ok_or_else(refusal)?;
        for function in &program.functions {
            let loops = expected
                .loops
                .iter()
                .filter(|item| item.function.stable_id == function.stable_id);
            let loop_count = loops.clone().count();
            let Some(resolved) = module
                .functions
                .iter()
                .find(|item| item.id.as_str() == function.stable_id)
            else {
                if loop_count == 0 {
                    continue;
                }
                return Err(refusal());
            };
            if loop_count == 0 {
                continue;
            }
            let mut pending = vec![&resolved.body];
            let mut matched_loops = 0;
            let mut actual_loops = 0;
            let mut matched_reads = 0;
            while let Some(expression) = pending.pop() {
                if matches!(&expression.kind, hir::ResolvedExprKind::Block { statements, .. }
                    if matches!(statements.first(), Some(hir::ResolvedStatement::Let { binding, .. }) if binding.name == "#for-borrow-length"))
                {
                    actual_loops += 1;
                }
                for item in loops.clone() {
                    if expression.id == hir::ExpressionId::new(&execution(function), &item.path) {
                        let hir::ResolvedExprKind::Block { statements, .. } = &expression.kind
                        else {
                            return Err(refusal());
                        };
                        let Some(hir::ResolvedStatement::Let {
                            value: length,
                            binding,
                            ..
                        }) = statements.first()
                        else {
                            return Err(refusal());
                        };
                        let hir::ResolvedExprKind::Call {
                            type_arguments,
                            args,
                            ..
                        } = &length.kind
                        else {
                            return Err(refusal());
                        };
                        if binding.name != "#for-borrow-length"
                            || !matches!(type_arguments.as_slice(), [element] if element_matches(element, item.declaration))
                            || !matches!(args.as_slice(), [source] if exact_place(source, &item.root))
                        {
                            return Err(refusal());
                        }
                        let Some(
                            hir::ResolvedStatement::While { body, .. }
                            | hir::ResolvedStatement::LexicalWhile { body, .. },
                        ) = statements.get(2)
                        else {
                            return Err(refusal());
                        };
                        let hir::ResolvedExprKind::Block { statements, .. } = &body.kind else {
                            return Err(refusal());
                        };
                        if !matches!(statements.first(), Some(hir::ResolvedStatement::Let { binding, .. }) if binding.id == item.index && binding.name == item.index_name)
                        {
                            return Err(refusal());
                        }
                        matched_loops += 1;
                    }
                }
                for read in expected
                    .reads
                    .iter()
                    .filter(|read| read.function.stable_id == function.stable_id)
                {
                    if expression.id != hir::ExpressionId::new(&execution(function), &read.path) {
                        continue;
                    }
                    let (element, field, args) = if read.copy {
                        let hir::ResolvedExprKind::Project { base, field } = &expression.kind
                        else {
                            return Err(refusal());
                        };
                        let hir::ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            args,
                            ..
                        } = &base.kind
                        else {
                            return Err(refusal());
                        };
                        if callee.as_str() != crate::vec_ops::GET_ID {
                            return Err(refusal());
                        }
                        let [element] = type_arguments.as_slice() else {
                            return Err(refusal());
                        };
                        (element, field, args)
                    } else {
                        let hir::ResolvedExprKind::VecFieldRead {
                            element,
                            field,
                            bytes,
                            args,
                        } = &expression.kind
                        else {
                            return Err(refusal());
                        };
                        if *bytes != read.bytes {
                            return Err(refusal());
                        }
                        (element, field, args)
                    };
                    if !element_matches(element, read.declaration)
                        || field.as_str() != read.field.stable_id
                        || !matches!(args.as_slice(), [vector, index] if exact_place(vector, &read.root) && exact_place(index, &read.index))
                    {
                        return Err(refusal());
                    }
                    matched_reads += 1;
                }
                hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
            }
            if actual_loops != loop_count
                || matched_loops != loop_count
                || matched_reads
                    != expected
                        .reads
                        .iter()
                        .filter(|read| read.function.stable_id == function.stable_id)
                        .count()
            {
                return Err(refusal());
            }
        }
    }
    Ok(())
}
