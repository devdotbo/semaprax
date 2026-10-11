//! Independently rederived never-reached clone payload work; no attached fact.
use super::{
    ResolvedExpr, ResolvedExprKind as K, ResolvedFunction, ResolvedStatement as S, ResolvedType,
};
use crate::hir::{OwnershipMode, ResolvedProgram, ValueId};
fn root(expr: &ResolvedExpr, id: &ValueId) -> bool {
    matches!(&expr.kind, K::Place(place) if place.root == *id && place.projections.is_empty())
}
fn walk<'a>(expr: &'a ResolvedExpr, mut visitor: impl FnMut(&'a ResolvedExpr)) -> bool {
    use crate::hir::lexical_exit::ChildCursor;
    type Frames<'a> = [Option<ChildCursor<'a>>; crate::cache_codec::MAX_DEPTH + 1];
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Frames<'_>>()) {
        return false;
    }
    let mut stack: Frames<'_> = [None; crate::cache_codec::MAX_DEPTH + 1];
    stack[0] = Some(ChildCursor::new(expr));
    let mut len = 1;
    let mut work = 65_536usize;
    while len != 0 {
        let frame = stack[len - 1].as_mut().expect("occupied empty-HIR frame");
        if !frame.is_started() {
            if work == 0 {
                return false;
            }
            work -= 1;
            visitor(frame.expression());
        }
        if let Some(child) = frame.next() {
            if len == stack.len() {
                return false;
            }
            stack[len] = Some(ChildCursor::new(child));
            len += 1;
        } else {
            stack[len - 1] = None;
            len -= 1;
        }
    }
    true
}
pub(crate) fn refused(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    target: &ResolvedExpr,
) -> bool {
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &target.kind
    else {
        return false;
    };
    let [element] = type_arguments.as_slice() else {
        return false;
    };
    let [carrier, index] = args.as_slice() else {
        return false;
    };
    let K::Place(place) = &carrier.kind else {
        return false;
    };
    if callee.as_str() != crate::vec_ops::CLONE_AT_ID
        // Place expressions retain the binding's Own mode. The exact
        // compiler-owned clone signature supplies the non-consuming borrow.
        || !matches!(carrier.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || !place.projections.is_empty()
        || index.ty != ResolvedType::Usize
        || !matches!(index.kind, K::Usize(_))
        || index.ownership != OwnershipMode::Value
        || !matches!(element, ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
        || super::super::super::owned_leaf_collection::layout(&program.declarations, element)
            .is_none()
        || target.ty != *element
        || target.ownership != OwnershipMode::Own
    {
        return false;
    }
    let K::Block { statements, .. } = &function.body.kind else {
        return false;
    };
    let Some(declaration) = statements.iter().position(|statement| matches!(statement,
        S::Let { binding, mutable: false, value, .. }
        if binding.id == place.root && binding.ty == carrier.ty && binding.ownership == OwnershipMode::Own
            && matches!(&binding.ty, ResolvedType::Nominal { declaration, arguments }
                if declaration.as_str() == crate::prelude::VEC_ID && arguments.as_slice() == std::slice::from_ref(element))
            && value.ty == binding.ty && value.ownership == OwnershipMode::Own
            && matches!(&value.kind, K::Call { callee, type_arguments, instance: None, args }
                if callee.as_str() == crate::vec_ops::WITH_CAPACITY_ID && type_arguments.as_slice() == std::slice::from_ref(element)
                    && matches!(args.as_slice(), [arg] if arg.ty == ResolvedType::Usize && arg.ownership == OwnershipMode::Value && matches!(arg.kind, K::Usize(value) if value <= crate::hir::owned_leaf_collection::capacity(&program.declarations, element)))))) else { return false; };
    let mut eligible = false;
    let complete = super::scan(
        function,
        &|op, element| super::super::element_mode(&program.declarations, op, element),
        |site| {
            let K::Call { args, .. } = &site.at.kind else {
                return;
            };
            if args.get(1).is_some_and(|arg| std::ptr::eq(arg, target)) {
                eligible |= statements[declaration + 1..].iter().any(|statement| {
                    (0..statement.child_count())
                        .filter_map(|i| statement.child(i))
                        .any(|expr| {
                            let mut found = false;
                            let complete = walk(expr, |node| found |= std::ptr::eq(node, target));
                            complete && found
                        })
                });
            }
        },
    );
    if !complete || !eligible {
        return false;
    }
    let mut declarations = 0usize;
    let mut references = 0usize;
    let mut allowed = 0usize;
    let mut valid = true;
    let complete = walk(&function.body, |expr| {
        if let K::Place(read) = &expr.kind {
            if read.root == place.root {
                references += 1;
                valid &= read.projections.is_empty()
                    && expr.ty == carrier.ty
                    && matches!(expr.ownership, OwnershipMode::Own | OwnershipMode::Borrow);
            }
        }
        if let K::Closure { captures, .. } = &expr.kind {
            valid &= !captures
                .iter()
                .any(|capture| root(&capture.value, &place.root));
        }
        if let K::Block { statements, .. } = &expr.kind {
            declarations += statements
                .iter()
                .filter(|s| matches!(s, S::Let { binding, .. } if binding.id == place.root))
                .count();
            valid &= !statements
                .iter()
                .any(|s| matches!(s, S::Assign { binding, .. } if binding.id == place.root));
        }
        if let K::Call {
            callee,
            type_arguments,
            instance: None,
            args,
        } = &expr.kind
        {
            if type_arguments.as_slice() == std::slice::from_ref(element)
                && ((callee.as_str() == crate::vec_ops::LEN_ID && args.len() == 1)
                    || (callee.as_str() == crate::vec_ops::CLONE_AT_ID
                        && args.len() == 2
                        && matches!(args[1].kind, K::Usize(_))))
                && args.first().is_some_and(|arg| root(arg, &place.root))
            {
                allowed += 1;
            }
        }
    });
    complete && valid && declarations == 1 && references != 0 && references == allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    fn selected(function: &ResolvedFunction) -> &ResolvedExpr {
        let mut selected = None;
        assert!(walk(&function.body, |expr| {
            if matches!(&expr.kind, K::Call { callee, .. } if callee.as_str() == crate::vec_ops::CLONE_AT_ID)
            {
                selected = Some(expr);
            }
        }));
        selected.expect("owning clone operand")
    }
    fn selected_mut(expression: &mut ResolvedExpr) -> Option<&mut ResolvedExpr> {
        if matches!(&expression.kind, K::Call { callee, .. } if callee.as_str()==crate::vec_ops::CLONE_AT_ID)
        {
            return Some(expression);
        }
        match &mut expression.kind {
            K::Block { statements, tail } => {
                for statement in statements {
                    let child = match statement {
                        S::Let { value, .. } | S::Assign { value, .. } => value,
                        S::While { body, .. } | S::LexicalWhile { body, .. } => body.as_mut(),
                        _ => continue,
                    };
                    if let Some(found) = selected_mut(child) {
                        return Some(found);
                    }
                }
                selected_mut(tail)
            }
            K::Call { args, .. } => args.iter_mut().find_map(selected_mut),
            K::Match { arms, .. } => arms.iter_mut().find_map(|arm| selected_mut(&mut arm.value)),
            _ => None,
        }
    }
    #[test]
    fn empty_clone_original_owned_place_does_not_grant_value_or_wrong_owner_authority() {
        let source = super::super::tests::SOURCE.replace(
            "words=vec_push<Row>(words,item)",
            "words=vec_push<Row>(words,vec_clone_at<Row>(later,0usize))",
        );
        let program =
            crate::hir::resolve(&crate::check(&source, "empty-carrier.spx").unwrap()).unwrap();
        for wrong_owner in [false, true] {
            let mut forged = program.clone();
            let result_id = forged.functions[0].result_id.clone();
            let target =
                selected_mut(&mut forged.functions[0].body).expect("actual lowered clone site");
            let K::Call { args, .. } = &mut target.kind else {
                unreachable!()
            };
            assert_eq!(args[0].ownership, OwnershipMode::Own);
            if wrong_owner {
                let K::Place(place) = &mut args[0].kind else {
                    unreachable!()
                };
                place.root = result_id;
            } else {
                args[0].ownership = OwnershipMode::Value;
            }
            let function = &forged.functions[0];
            assert!(!refused(&forged, function, selected(function)));
            assert!(crate::hir::validate(&forged).is_err());
        }
    }
    #[test]
    fn refusal_replays_current_nominal_owner_constructor_and_not_attached_or_forged_call() {
        let source = super::super::tests::SOURCE.replace(
            "words=vec_push<Row>(words,item)",
            "words=vec_push<Row>(words,vec_clone_at<Row>(later,0usize))",
        );
        let parsed = crate::check(&source, "empty-refusal.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let function = &program.functions[0];
        let K::Call { args, .. } = &selected(function).kind else {
            unreachable!()
        };
        assert_eq!(args[0].ownership, OwnershipMode::Own);
        assert_eq!(
            crate::vec_ops::resolved_params_in(
                &program.declarations,
                crate::vec_ops::VecOp::CloneAt,
                &selected(function).ty
            )[0]
            .ownership,
            OwnershipMode::Borrow
        );
        assert!(refused(&program, function, selected(function)));
        crate::hir::analyze_byte_data_capacity(&program).unwrap();
        crate::hir::validate(&program).unwrap();
        let alternate = source
            .replace(
                "let later=vec_with_capacity<Row>(1usize)",
                "let later=vec_with_capacity<Row>(3usize)",
            )
            .replace("later,0usize", "later,7usize");
        let alternate =
            crate::hir::resolve(&crate::check(&alternate, "alternate-empty-refusal.spx").unwrap())
                .unwrap();
        assert!(refused(
            &alternate,
            &alternate.functions[0],
            selected(&alternate.functions[0])
        ));
        crate::hir::validate(&alternate).unwrap();
        let canonical = crate::format::canonical(&parsed);
        assert_eq!(
            canonical,
            crate::format::canonical(&crate::check(&canonical, "canonical-empty.spx").unwrap())
        );
        assert_eq!(
            crate::graph::graph_schema(&program).unwrap(),
            "semaprax.graph.v79"
        );
        let graph = crate::graph::to_json(&parsed).unwrap();
        crate::graph::verify_json(&parsed, &graph).unwrap();
        let bytes = crate::cache_codec::encode(&function.cleanup_plan).unwrap();
        let restored: crate::cleanup_plan::CleanupPlan =
            crate::cache_codec::decode(&bytes).unwrap();
        assert_eq!(restored, function.cleanup_plan);
        for mutation in 0..4 {
            let mut forged = program.clone();
            let K::Block { statements, .. } = &mut forged.functions[0].body.kind else {
                panic!("root");
            };
            let S::Let {
                binding,
                mutable,
                value,
                ..
            } = &mut statements[1]
            else {
                panic!("empty owner");
            };
            match mutation {
                0 => *mutable = true,
                1 => binding.ty = ResolvedType::String,
                2 => {
                    let K::Call { callee, .. } = &mut value.kind else {
                        panic!("constructor");
                    };
                    *callee = crate::hir::DeclarationId::new(crate::vec_ops::PUSH_ID);
                }
                3 => {
                    let K::Call { args, .. } = &mut value.kind else {
                        panic!("constructor");
                    };
                    args[0].kind = K::Usize(crate::vec_ops::MAX_CAPACITY + 1);
                }
                _ => unreachable!(),
            }
            let function = &forged.functions[0];
            assert!(
                !refused(&forged, function, selected(function)),
                "forgery {mutation}"
            );
        }
        // Copying an otherwise identical call does not carry site authority.
        assert!(!refused(&program, function, &selected(function).clone()));
    }
}
