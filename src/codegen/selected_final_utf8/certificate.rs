use super::*;
use crate::{
    cleanup_plan::{StatusLane, StatusProducer},
    hir::{
        self, OwnershipMode, ResolvedExpr, ResolvedExprKind as K, ResolvedType as T,
        ResolvedTypeDeclarationKind as D,
    },
    variant_layout::{VariantLayout, VariantTarget},
};
use serde_json::json;
use sha2::{Digest, Sha256};
const WORK: usize = 131_072;
pub(in crate::codegen) struct Selection {
    pub(in crate::codegen) bytes: Vec<u8>,
    pub(in crate::codegen) entry: DeclarationId,
    pub(in crate::codegen) factory: DeclarationId,
    pub(in crate::codegen) disposer: DeclarationId,
    pub(in crate::codegen) functions: Vec<DeclarationId>,
    pub(in crate::codegen) encoded_case: DeclarationId,
    pub(in crate::codegen) text_field: DeclarationId,
    pub(in crate::codegen) refused_case: DeclarationId,
    pub(in crate::codegen) required_field: DeclarationId,
}
impl Selection {
    /// This transient emission view is NOT a new valid public HIR program or
    /// graph. Its missing ordinary main must never be exported/replayed as HIR.
    pub(in crate::codegen) fn emission_view(&self, original: &ResolvedProgram) -> ResolvedProgram {
        ResolvedProgram {
            module: original.module.clone(),
            permits: original.permits.clone(),
            agents: original.agents.clone(),
            entrypoint: original.entrypoint.clone(),
            declarations: original.declarations.clone(),
            types: original.types.clone(),
            interfaces: original.interfaces.clone(),
            function_templates: Vec::new(),
            function_instances: Vec::new(),
            functions: original
                .functions
                .iter()
                .filter(|f| self.functions.contains(&f.id))
                .cloned()
                .collect(),
        }
    }
}
pub(super) fn derive(
    program: &ResolvedProgram,
    entry: &DeclarationId,
    factory: &DeclarationId,
    disposer: &DeclarationId,
) -> Result<Selection, Diagnostic> {
    hir::validate(program)?;
    let inventory = crate::codegen::checked_final_utf8_preflight::inspect(program, entry)?;
    let root = program
        .functions
        .iter()
        .find(|f| &f.id == entry)
        .ok_or_else(|| error("missing monomorphic root"))?;
    if root.params.len() != 2
        || root.params[0].ownership != OwnershipMode::Borrow
        || !matches!(&root.params[0].ty,T::Nominal{arguments,..} if arguments.is_empty())
        || root.params[1].ty != T::Usize
        || root.params[1].ownership != OwnershipMode::Value
    {
        return Err(error("root requires (borrow monomorphic Report, usize)"));
    }
    let mut work = inventory.work;
    // The emission view clones only selected function bodies, but retains the
    // exact original declaration/layout namespace. Charge those inventories too.
    charge(&mut work, program.declarations.declarations().len())?;
    charge(&mut work, program.types.len())?;
    charge(&mut work, program.interfaces.len())?;
    charge(&mut work, program.agents.len())?;
    charge(&mut work, program.permits.len())?;
    if entry == factory || entry == disposer || factory == disposer {
        return Err(error(
            "selected function roles must have distinct identities",
        ));
    }
    let producer_inventory =
        crate::codegen::checked_final_utf8_preflight::inspect(program, factory)?;
    let producer = program
        .functions
        .iter()
        .find(|f| &f.id == factory)
        .ok_or_else(|| error("missing exact checked Report factory"))?;
    let dispose = program
        .functions
        .iter()
        .find(|f| &f.id == disposer)
        .ok_or_else(|| error("missing exact checked Report disposer"))?;
    if producer.params.len() > 1
        || producer
            .params
            .iter()
            .any(|p| p.ownership != OwnershipMode::Value || p.ty != T::Usize)
        || producer.return_type != root.params[0].ty
    {
        return Err(error("checked Report factory requires zero/one Copy usize input and exact owning Report result"));
    }
    if dispose.params.len() != 1
        || dispose.params[0].ownership != OwnershipMode::Own
        || dispose.params[0].ty != root.params[0].ty
        || dispose.return_type != T::I64
        || !dispose.requires.is_empty()
        || !dispose.ensures.is_empty()
        || !matches!(&dispose.body.kind,K::Block{statements,tail} if statements.is_empty() && matches!(tail.kind,K::Int(0)))
    {
        return Err(error(
            "Report disposer must be the exact checked own-Report constant-zero cleanup function",
        ));
    }
    charge(&mut work, producer_inventory.work)?;
    for id in producer_inventory
        .functions
        .iter()
        .chain(std::iter::once(disposer))
    {
        charge(&mut work, program.functions.len())?;
        let f = program
            .functions
            .iter()
            .find(|f| &f.id == id)
            .ok_or_else(|| error("missing Report provenance function"))?;
        if !f.effects.is_empty() || f.yields.is_some() || f.cleanup_plan.lexical_control.is_some() {
            return Err(error(
                "Report provenance closure must be ordinary pure monomorphic source",
            ));
        }
        for expr in f
            .requires
            .iter()
            .chain(std::iter::once(&f.body))
            .chain(&f.ensures)
        {
            walk(expr, &mut work, |expr, _| match &expr.kind {
                K::Call {
                    callee, instance, ..
                } if instance.is_none()
                    && (producer_inventory.functions.contains(callee)
                        || crate::string_ops::by_id(callee.as_str()).is_some()
                        || crate::byte_ops::by_id(callee.as_str()).is_some()
                        || crate::vec_ops::by_id(callee.as_str()).is_some()) =>
                {
                    Ok(())
                }
                K::Call { .. }
                | K::Invoke { .. }
                | K::FunctionReference { .. }
                | K::Closure { .. }
                | K::NativeRustImportCall(_)
                | K::HostCommandCall(_)
                | K::Yield { .. } => Err(error(
                    "unexpanded execution in checked Report provenance closure",
                )),
                _ => Ok(()),
            })?;
        }
    }

    let T::Nominal {
        declaration: input_id,
        ..
    } = &root.params[0].ty
    else {
        return Err(error("input is not a record"));
    };
    if !program.types.iter().any(|d| {
        &d.id == input_id && matches!(d.kind, D::Record { .. }) && d.type_parameters.is_empty()
    }) {
        return Err(error(
            "input requires an exact monomorphic record declaration",
        ));
    }
    if !matches!(&root.return_type,T::Nominal { arguments,.. } if arguments.is_empty()) {
        return Err(error("result requires a monomorphic outcome declaration"));
    }
    // No extra projected Bytes pointer ABI is hidden in the borrowed Report.
    if contains(program, &root.params[0].ty, &T::Bytes, 0, &mut work)? {
        return Err(error(
            "borrowed Report containing Bytes requires a separate input ABI",
        ));
    }
    let layout = VariantLayout::for_type(program, VariantTarget::Native64, &root.return_type)?;
    layout.validate(program)?;
    if layout.cases.len() != 2 || layout.cases.iter().any(|c| c.fields.len() != 1) {
        return Err(error(
            "result requires exactly String and usize outcome cases",
        ));
    }
    let encoded = layout
        .cases
        .iter()
        .find(|c| c.fields[0].ty == T::String)
        .ok_or_else(|| error("result has no exact String case"))?;
    let refused = layout
        .cases
        .iter()
        .find(|c| c.fields[0].ty == T::Usize)
        .ok_or_else(|| error("result has no exact usize case"))?;
    let mut constructor = None;
    let mut tail_work = 0;
    tail(
        &root.body,
        &encoded.case,
        &encoded.fields[0].field,
        &refused.case,
        &refused.fields[0].field,
        0,
        &mut tail_work,
        &mut constructor,
    )?;
    charge(&mut work, tail_work)?;
    let constructor =
        constructor.ok_or_else(|| error("missing direct final constructor in outcome tail"))?;
    let mut constructors = 0;
    for id in &inventory.functions {
        charge(&mut work, program.functions.len())?;
        let function = program
            .functions
            .iter()
            .find(|f| &f.id == id)
            .ok_or_else(|| error("closure function disappeared"))?;
        if !function.effects.is_empty()
            || function.yields.is_some()
            || function.cleanup_plan.lexical_control.is_some()
        {
            return Err(error(
                "effectful, yielding or lexical-exit closure is outside this profile",
            ));
        }
        if id != entry && contains(program, &function.return_type, &T::String, 0, &mut work)? {
            return Err(error(
                "helper may not return an owned String-containing carrier",
            ));
        }
        for parameter in &function.params {
            if contains(program, &parameter.ty, &T::StringMap, 0, &mut work)?
                || parameter.ty.is_callable()
            {
                return Err(error("map/callable parameter outside closed selected ABI"));
            }
            if parameter.ownership != OwnershipMode::Borrow
                && contains(program, &parameter.ty, &T::String, 0, &mut work)?
            {
                return Err(error(
                    "owning String-containing parameters cannot enter selected cleanup",
                ));
            }
        }
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            walk(expression, &mut work, |expr, work| {
                match &expr.kind {
                    K::Call {
                        callee, instance, ..
                    } => {
                        if instance.is_some() {
                            return Err(error("generic execution is outside the selected closure"));
                        }
                        if inventory.functions.contains(callee) {
                            return Ok(());
                        }
                        if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                            if op == crate::string_ops::StringOp::FromUtf8 {
                                let K::Call { args, .. } = &expr.kind else {
                                    unreachable!("matched call")
                                };
                                if args.len() != 1
                                    || !matches!(&args[0].kind,K::Call{callee,instance:None,args,type_arguments} if type_arguments.is_empty() && crate::byte_ops::by_id(callee.as_str())==Some(crate::byte_ops::ByteOp::BytesAsSlice) && args.len()==1)
                                {
                                    return Err(error("final constructor requires the exact bounded owned-Bytes view"));
                                }
                                if id != entry || expr.id != constructor {
                                    return Err(error("constructor is outside the single authenticated result tail"));
                                }
                                constructors += 1;
                                charge(work, function.cleanup_plan.status_sources.len())?;
                                let mut sources =
                                    function.cleanup_plan.status_sources.iter().filter(|s| {
                                        s.id.expression == expr.id
                                            && s.id.lane == StatusLane::OperationFailure
                                    });
                                let source = sources.next();
                                if sources.next().is_some()
                                    || !matches!(source.map(|s|&s.producer),Some(StatusProducer::PropagatedCall{callee:actual}) if actual==callee)
                                {
                                    return Err(error(
                                        "constructor has no exact replayed failure producer",
                                    ));
                                }
                            } else if op.return_type() == T::String
                                || matches!(
                                    op,
                                    crate::string_ops::StringOp::MapNew
                                        | crate::string_ops::StringOp::MapAdd
                                        | crate::string_ops::StringOp::MapSet
                                        | crate::string_ops::StringOp::MapRemove
                                        | crate::string_ops::StringOp::FileReadText
                                )
                            {
                                return Err(error(
                                    "another allocating String/map operation is forbidden",
                                ));
                            }
                        } else if crate::byte_ops::by_id(callee.as_str()).is_some() {
                            // Full HIR replay already authenticated exact operation, type,
                            // capacity, loan and same-owner transfer arguments.
                        } else if matches!(
                            crate::vec_ops::by_id(callee.as_str()),
                            Some(
                                crate::vec_ops::VecOp::Len
                                    | crate::vec_ops::VecOp::Capacity
                                    | crate::vec_ops::VecOp::Get
                            )
                        ) {
                            if expr.ownership == OwnershipMode::Own
                                && contains(program, &expr.ty, &T::String, 0, work)?
                            {
                                return Err(error("Vec read may materialize String owners"));
                            }
                        } else {
                            return Err(error(
                                "unexpanded intrinsic/import/callback in selected closure",
                            ));
                        }
                    }
                    K::BorrowedUtf8Literal(_) => {
                        return Err(error("static borrowed UTF-8 is outside the selected final-UTF8 certificate profile"));
                    }
                    K::String(_) | K::LiteralFormat { .. } => {
                        return Err(error("literal/format String allocation is forbidden"))
                    }
                    K::Place(_) | K::Project { .. } | K::VecFieldRead { .. }
                        if expr.ownership == OwnershipMode::Own
                            && contains(program, &expr.ty, &T::String, 0, work)? =>
                    {
                        return Err(error("owning String-containing Place may clone"))
                    }
                    K::ConstructRecord { .. }
                        if contains(program, &expr.ty, &T::String, 0, work)? =>
                    {
                        return Err(error("selected closure cannot create owning input records"))
                    }
                    K::Int(_)
                    | K::Int32(_)
                    | K::Char(_)
                    | K::Uint8(_)
                    | K::Usize(_)
                    | K::ArrayU8(_)
                    | K::RepeatArrayU8 { .. }
                    | K::Bool(_)
                    | K::Float32(_)
                    | K::Float64(_)
                    | K::Place(_)
                    | K::BorrowPlace { .. }
                    | K::ByteRange { .. }
                    | K::VecFieldRead { .. }
                    | K::Unary { .. }
                    | K::Binary { .. }
                    | K::Block { .. }
                    | K::If { .. }
                    | K::ConstructRecord { .. }
                    | K::ConstructVariant { .. }
                    | K::Match { .. }
                    | K::Project { .. } => {}
                    _ => return Err(error("unsupported expression boundary in selected closure")),
                }
                Ok(())
            })?;
        }
    }
    if constructors != 1 {
        return Err(error(
            "selected closure requires one exact final UTF8 constructor",
        ));
    }
    let mut emission_functions = inventory.functions.clone();
    for id in producer_inventory
        .functions
        .iter()
        .chain(std::iter::once(disposer))
    {
        charge(&mut work, emission_functions.len())?;
        if !emission_functions.contains(id) {
            if emission_functions.len() == 256 {
                return Err(error(
                    "combined selected provenance closure capacity exceeded",
                ));
            }
            emission_functions.push(id.clone());
        }
    }
    if producer_inventory.functions.contains(entry) {
        return Err(error(
            "Report factory may not invoke selected final encoder",
        ));
    }
    let input_layout = crate::aggregate_layout::AggregateLayout::for_type(
        program,
        crate::aggregate_layout::AggregateTarget::Native64,
        &root.params[0].ty,
    )?;
    input_layout.validate(program)?;
    let input_digest = format!("{:x}", crate::digest_hex::LowerHex(input_layout.digest()));
    let result_digest = format!("{:x}", crate::digest_hex::LowerHex(layout.digest()));
    let runtime_digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(include_bytes!("runtime.c")))
    );
    let graph = crate::graph::to_hir_json(program, super::PROFILE)?;
    let graph_digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(graph.as_bytes()))
    );
    let bytes = serde_json::to_vec(&json!({"schema":super::PROFILE,"runtime_sha256":runtime_digest,"hir_graph_sha256":graph_digest,"entry":entry.as_str(),"factory":factory.as_str(),"disposer":disposer.as_str(),"input_authority":"checked-source-report-handle.v1","functions":emission_functions.iter().map(DeclarationId::as_str).collect::<Vec<_>>(),"constructor":constructor.as_str(),"status_lane":"operation_failure","allocation_status_backing":"record-before-callback.v1","minimum_free_status_entries":2,"normalized_cases":[["semaprax.allocation.v1",1,"Adapter",false],["semaprax.allocation.v1",2,"Adapter",false],["semaprax.convert.v1",1,"Adapter",false]],"input_layout_sha256":input_digest,"result_layout_sha256":result_digest,"input":root.params[0].ty.identity_key(),"result":root.return_type.identity_key(),"encoded_case":encoded.case.as_str(),"text_field":encoded.fields[0].field.as_str(),"refused_case":refused.case.as_str(),"required_field":refused.fields[0].field.as_str(),"target":"native64","output":"opaque-handle-lease.v1","allocator":"retained-context-module-callback-pair.v1","module_binding":"distinct-address-emission-object.v1","handle_capacity":8,"payload_limit":131072,"work":work})).map_err(|_|error("certificate serialization"))?;
    if bytes.len() > 65_536 {
        return Err(error("selected certificate capacity exceeded"));
    }
    Ok(Selection {
        bytes,
        entry: entry.clone(),
        factory: factory.clone(),
        disposer: disposer.clone(),
        functions: emission_functions,
        encoded_case: encoded.case.clone(),
        text_field: encoded.fields[0].field.clone(),
        refused_case: refused.case.clone(),
        required_field: refused.fields[0].field.clone(),
    })
}
fn charge(work: &mut usize, n: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(n)
        .filter(|v| *v <= WORK)
        .ok_or_else(|| error("selected closure work capacity exceeded"))?;
    Ok(())
}
fn contains(
    program: &ResolvedProgram,
    ty: &T,
    needle: &T,
    depth: usize,
    work: &mut usize,
) -> Result<bool, Diagnostic> {
    charge(work, 1)?;
    if depth > 32 {
        return Err(error("selected carrier depth exceeded"));
    }
    if ty == needle {
        return Ok(true);
    }
    if let T::Nominal {
        declaration,
        arguments,
    } = ty
    {
        for arg in arguments {
            if contains(program, arg, needle, depth + 1, work)? {
                return Ok(true);
            }
        }
        charge(work, program.types.len())?;
        if let Some(decl) = program.types.iter().find(|d| &d.id == declaration) {
            match &decl.kind {
                D::Record { fields, .. } | D::Class { fields, .. } => {
                    for field in fields {
                        if contains(program, &field.ty, needle, depth + 1, work)? {
                            return Ok(true);
                        }
                    }
                }
                D::Variant { cases } => {
                    for field in cases.iter().flat_map(|c| &c.fields) {
                        if contains(program, &field.ty, needle, depth + 1, work)? {
                            return Ok(true);
                        }
                    }
                }
                D::Resource { .. } => return Err(error("resource carrier outside selected ABI")),
            }
        }
    }
    Ok(false)
}
fn walk(
    root: &ResolvedExpr,
    work: &mut usize,
    mut visit: impl FnMut(&ResolvedExpr, &mut usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
    stack[0] = Some((root, 0usize));
    let mut len = 1;
    while len != 0 {
        let (expr, index) = stack[len - 1].expect("occupied frame");
        charge(work, index + 1)?;
        if index == 0 {
            visit(expr, work)?;
        }
        if let Some(child) = hir::lexical_exit::child(expr, index) {
            if len == stack.len() {
                return Err(error("selected closure depth exceeded"));
            }
            stack[len - 1] = Some((expr, index + 1));
            stack[len] = Some((child, 0));
            len += 1;
        } else {
            len -= 1;
            stack[len] = None;
        }
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)] // Exact nominal case/field identities plus independently charged traversal state.
fn tail(
    expr: &ResolvedExpr,
    encoded: &DeclarationId,
    text: &DeclarationId,
    refused: &DeclarationId,
    required: &DeclarationId,
    depth: usize,
    work: &mut usize,
    site: &mut Option<hir::ExpressionId>,
) -> Result<(), Diagnostic> {
    charge(work, 1)?;
    if depth > crate::cache_codec::MAX_DEPTH {
        return Err(error("selected tail depth exceeded"));
    }
    match &expr.kind {
        K::Block { tail: next, .. } => tail(
            next,
            encoded,
            text,
            refused,
            required,
            depth + 1,
            work,
            site,
        ),
        K::If {
            then_branch,
            else_branch,
            ..
        } => {
            tail(
                then_branch,
                encoded,
                text,
                refused,
                required,
                depth + 1,
                work,
                site,
            )?;
            tail(
                else_branch,
                encoded,
                text,
                refused,
                required,
                depth + 1,
                work,
                site,
            )
        }
        K::ConstructVariant { case, fields, .. }
            if case == encoded && fields.len() == 1 && &fields[0].field == text =>
        {
            if !matches!(&fields[0].value.kind,K::Call{callee,instance:None,..} if crate::string_ops::by_id(callee.as_str())==Some(crate::string_ops::StringOp::FromUtf8))
                || site.is_some()
            {
                return Err(error(
                    "Encoded requires the single direct final constructor",
                ));
            }
            *site = Some(fields[0].value.id.clone());
            Ok(())
        }
        K::ConstructVariant { case, fields, .. }
            if case == refused && fields.len() == 1 && &fields[0].field == required =>
        {
            Ok(())
        }
        _ => Err(error(
            "result tail must construct the authenticated outcome directly",
        )),
    }
}
