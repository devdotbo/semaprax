//! Borrowed field permissions confined to one independently checked owned arm.
use super::*;
type Fields<'a> = &'a [crate::hir::ResolvedRecordMatchPatternField];
type Entry<'a> = (&'a ResolvedExpr, Option<Fields<'a>>);

pub(super) struct Pending<'a> {
    legacy: Vec<&'a ResolvedExpr>,
    entries: Vec<Entry<'a>>,
    current: Option<Fields<'a>>,
    metered: bool,
    error: Option<Diagnostic>,
}
impl<'a> Pending<'a> {
    pub(super) fn new(expression: &'a ResolvedExpr, metered: bool) -> Self {
        let admitted =
            !metered || crate::bounded_output::reserve_active_required(std::mem::size_of::<Self>());
        let mut result = Self {
            legacy: if metered {
                Vec::new()
            } else {
                vec![expression]
            },
            entries: Vec::new(),
            current: None,
            metered,
            error: (!admitted)
                .then(|| hir_error("owned iterator field-scope carrier exceeds its bound")),
        };
        if metered {
            result.push(expression);
        }
        result
    }
    pub(super) fn pop(&mut self) -> Option<&'a ResolvedExpr> {
        if self.error.is_some() {
            return None;
        }
        if !self.metered {
            return self.legacy.pop();
        }
        let (expression, fields) = self.entries.pop()?;
        self.current = fields;
        Some(expression)
    }
    pub(super) fn push(&mut self, expression: &'a ResolvedExpr) {
        self.push_scoped(expression, self.current);
    }
    pub(super) fn extend(&mut self, values: impl IntoIterator<Item = &'a ResolvedExpr>) {
        for expression in values {
            self.push(expression);
        }
    }
    pub(super) fn push_scoped(&mut self, expression: &'a ResolvedExpr, fields: Option<Fields<'a>>) {
        if self.error.is_some() {
            return;
        }
        if !self.metered {
            self.legacy.push(expression);
            return;
        }
        if self.entries.len() == self.entries.capacity() && self.metered {
            let capacity = self.entries.capacity().max(1).checked_mul(2);
            let bytes =
                capacity.and_then(|count| count.checked_mul(std::mem::size_of::<Entry<'a>>()));
            if capacity.is_none_or(|count| count > 1_048_576)
                || bytes.is_none_or(|bytes| !crate::bounded_output::reserve_active_required(bytes))
            {
                self.error = Some(hir_error(
                    "owned iterator field-scope frontier exceeds its bound",
                ));
                return;
            }
            let capacity = capacity.unwrap();
            if self
                .entries
                .try_reserve_exact(capacity - self.entries.len())
                .is_err()
                || self.entries.capacity() != capacity
            {
                self.error = Some(hir_error(
                    "owned iterator field-scope frontier allocation refused",
                ));
                return;
            }
        }
        self.entries.push((expression, fields));
    }
    pub(super) fn admits(&self, expression: &ResolvedExpr) -> bool {
        let ResolvedExprKind::Place(place) = &expression.kind else {
            return false;
        };
        expression.ty == ResolvedType::Bytes
            && expression.ownership == OwnershipMode::Own
            && place.projections.is_empty()
            && self.current.is_some_and(|fields| fields.iter().any(|field| {
                matches!(&field.pattern, crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding)
                    if binding.id == place.root && binding.ty == expression.ty
                        && binding.ownership == OwnershipMode::Own)
            }))
    }
    pub(super) fn finish(self) -> Result<(), Diagnostic> {
        self.error.map_or(Ok(()), Err)
    }
}

pub(super) fn fields<'a>(
    declarations: &crate::hir::DeclarationIndex,
    item: &ResolvedBinding,
    arm: &'a crate::hir::ResolvedMatchArm,
) -> Result<Fields<'a>, Diagnostic> {
    let crate::hir::ResolvedMatchPattern::Record {
        record,
        instance,
        fields,
    } = &arm.pattern
    else {
        return Err(hir_error(
            "owned iterator arm is not its exact record pattern",
        ));
    };
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &item.ty
    else {
        return Err(hir_error("owned iterator item is not nominal"));
    };
    let declared = declarations
        .record_fields(record)
        .ok_or_else(|| hir_error("owned iterator arm has no current record fields"))?;
    if item.ownership != OwnershipMode::Own
        || arm.guard.is_some()
        || instance != &item.ty
        || record != declaration
        || !arguments.is_empty()
        || fields.len() != declared.len()
        || fields.len() > 8
    {
        return Err(hir_error(
            "owned iterator arm differs from its exact item layout",
        ));
    }
    for (index, field) in fields.iter().enumerate() {
        let actual = declared
            .iter()
            .find(|actual| actual.id == field.field)
            .ok_or_else(|| hir_error("owned iterator pattern has a foreign field"))?;
        if fields[..index]
            .iter()
            .any(|earlier| earlier.field == field.field)
        {
            return Err(hir_error("owned iterator pattern repeats a field"));
        }
        match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                let expected = if matches!(actual.ty, ResolvedType::String | ResolvedType::Bytes) {
                    OwnershipMode::Own
                } else if crate::hir::is_scalar_resolved_type(&actual.ty) {
                    OwnershipMode::Value
                } else {
                    return Err(hir_error(
                        "owned iterator field is outside the flat leaf profile",
                    ));
                };
                if binding.ty != actual.ty || binding.ownership != expected
                    || binding.id == item.id
                    || fields[..index].iter().any(|earlier| matches!(&earlier.pattern,
                        crate::hir::ResolvedRecordMatchFieldPattern::Binding(previous) if previous.id == binding.id)) {
                    return Err(hir_error("owned iterator field binding has a foreign type, mode or identity"));
                }
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Record { .. } => {
                return Err(hir_error("owned iterator arm has a nested field pattern"));
            }
        }
    }
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"module field_scope;
@id("row") record Row {@id("row.n") n:i64,@id("row.text") text:string,@id("row.bytes") bytes:Bytes,}
@id("output") record Output {@id("output.n") n:i64,@id("output.text") text:string,@id("output.bytes") bytes:Bytes,}
@id("main") fn main()->i64 {
 let inputs=vec_push<Row>(vec_with_capacity<Row>(1usize),Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 let mut outputs=vec_with_capacity<Output>(1usize);
 for own item in vec_into_iter<Row>(inputs) {
  let _=match own item {Row{n,text,bytes}=>{if n==7 {outputs=vec_push<Output>(outputs,Output{n:n,text:text,bytes:bytes});}0},};0
 }
 if vec_len<Output>(outputs)==1usize {29}else{0}
}"#;
    fn program() -> crate::hir::ResolvedProgram {
        crate::hir::resolve(&crate::check(SOURCE, "field-scope.spx").unwrap()).unwrap()
    }
    fn find(expression: &ResolvedExpr) -> Option<(&ResolvedExpr, &crate::hir::ResolvedMatchArm)> {
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Match {
                mode: ResolvedMatchMode::Own,
                scrutinee,
                arms,
                ..
            } = &expression.kind
            {
                if arms.len() == 1
                    && matches!(
                        arms[0].pattern,
                        crate::hir::ResolvedMatchPattern::Record { .. }
                    )
                {
                    return Some((scrutinee, &arms[0]));
                }
            }
            pending.extend(crate::hir::lexical_exit::ChildCursor::new(expression));
        }
        None
    }
    #[test]
    fn exact_owned_field_permission_expires_and_rejects_foreign_layout_type_mode_and_root() {
        let program = program();
        crate::hir::validate(&program).unwrap();
        let (scrutinee, arm) = find(&program.functions[0].body).unwrap();
        let ResolvedExprKind::Place(place) = &scrutinee.kind else {
            panic!("owned item place")
        };
        let item = ResolvedBinding {
            id: place.root.clone(),
            name: "item".into(),
            ownership: scrutinee.ownership,
            ty: scrutinee.ty.clone(),
            span: scrutinee.span,
        };
        let exact = fields(&program.declarations, &item, arm).unwrap();
        let binding = exact
            .iter()
            .find_map(|field| match &field.pattern {
                crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding)
                    if binding.ty == ResolvedType::Bytes =>
                {
                    Some(binding)
                }
                _ => None,
            })
            .unwrap();
        let mut value = scrutinee.clone();
        value.ty = binding.ty.clone();
        if let ResolvedExprKind::Place(place) = &mut value.kind {
            place.root = binding.id.clone();
        }
        let mut pending = Pending::new(scrutinee, true);
        pending.push_scoped(&value, Some(exact));
        assert!(std::ptr::eq(pending.pop().unwrap(), &value));
        assert!(pending.admits(&value));
        pending.pop().unwrap();
        assert!(
            !pending.admits(&value),
            "permission cannot escape the selected arm"
        );
        for mutation in 0..4 {
            let mut forged = arm.clone();
            let crate::hir::ResolvedMatchPattern::Record {
                fields: pattern, ..
            } = &mut forged.pattern
            else {
                unreachable!()
            };
            let field = pattern.iter_mut().find(|field| matches!(&field.pattern,
                crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) if binding.ty == ResolvedType::Bytes)).unwrap();
            match mutation {
                0 => field.field = crate::hir::DeclarationId::new("foreign.field"),
                _ => {
                    let crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) =
                        &mut field.pattern
                    else {
                        unreachable!()
                    };
                    match mutation {
                        1 => binding.ty = ResolvedType::String,
                        2 => binding.ownership = OwnershipMode::Borrow,
                        _ => binding.id = item.id.clone(),
                    }
                }
            }
            assert!(
                fields(&program.declarations, &item, &forged).is_err(),
                "mutation {mutation}"
            );
        }
        let ResolvedExprKind::Place(place) = &mut value.kind else {
            unreachable!()
        };
        place.root = item.id.clone();
        let mut pending = Pending::new(scrutinee, true);
        pending.push_scoped(&value, Some(exact));
        pending.pop().unwrap();
        assert!(!pending.admits(&value));
    }
    #[test]
    fn scoped_frontier_precharges_full_growth_exact_and_one_short() {
        let program = program();
        let expression = &program.functions[0].body;
        let exercise = || {
            let mut pending = Pending::new(expression, true);
            for _ in 0..8 {
                pending.push(expression);
            }
            while pending.pop().is_some() {}
            pending.finish()
        };
        let (result, refused, exact) =
            crate::bounded_output::with_limit_usage(usize::MAX, exercise);
        result.unwrap();
        assert!(!refused);
        assert!(exact > 0);
        let (result, refused, used) = crate::bounded_output::with_limit_usage(exact, exercise);
        result.unwrap();
        assert!(!refused);
        assert_eq!(used, exact);
        let (result, refused, _) = crate::bounded_output::with_limit_usage(exact - 1, exercise);
        assert_eq!(result.unwrap_err().code, "SPX-H006");
        assert!(refused);
    }
}
