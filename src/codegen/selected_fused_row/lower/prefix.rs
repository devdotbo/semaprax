//! Construction prefix bound to the selection's exact borrowed sites.
//!
//! Row operands keep their original left-to-right preparation and the plan's
//! staging transitions apply at each operand. The first original Bytes
//! operand, after every earlier row operand, allocates the admitted tables,
//! consumes the admitted ordinary hold and allocates the backing; nothing
//! there can refuse. Each row publishes before the next issue; ordinary Vec
//! allocation and capacity failures leave through their plan exits.
use super::*;
use crate::cleanup_plan::StorageId;

impl Lower<'_, '_> {
    pub(super) fn prefix(&mut self) -> Result<(), Diagnostic> {
        let sites = self.sites;
        let K::Block { statements, .. } = &sites.function.body.kind else {
            return Err(error("selected function body is not a block"));
        };
        let [S::Let {
            binding: vector,
            mutable: true,
            value: allocation,
            ..
        }, S::Let {
            binding: counter,
            mutable: true,
            value: start,
            ..
        }, S::While {
            condition, body, ..
        }, suffix @ ..] = statements.as_slice()
        else {
            return Err(error("selected function prefix drifted from the selection"));
        };
        if !std::ptr::eq(vector, sites.vector)
            || !std::ptr::eq(counter, sites.counter)
            || !std::ptr::eq(allocation, sites.allocation)
            || !std::ptr::eq(&**condition, sites.condition)
            || !std::ptr::eq(&**body, sites.body)
            || !std::ptr::eq(suffix, sites.suffix)
        {
            return Err(error(
                "selected prefix sites are not the selection's borrowed nodes",
            ));
        }
        let K::Call {
            callee,
            instance: None,
            args,
            ..
        } = &allocation.kind
        else {
            return Err(error("selected destination is not vec_with_capacity"));
        };
        let [capacity] = args.as_slice() else {
            return Err(error("selected destination has the wrong arity"));
        };
        let count = sites.rows;
        if callee.as_str() != crate::vec_ops::WITH_CAPACITY_ID
            || !matches!(capacity.kind, K::Usize(value) if value == u64::from(count))
        {
            return Err(error(
                "selected destination capacity is not the selection's row count",
            ));
        }
        let rows = self.bind(vector, Kind::Rows)?;
        self.register(
            StorageId::Value(vector.id.clone()),
            Physical::Rows(rows.clone()),
        )?;
        self.register(
            StorageId::Temporary(allocation.id.clone()),
            Physical::Rows(rows.clone()),
        )?;
        let failure = self.fail(
            &allocation.id,
            "spx_status = spx_vec_failure(spx_ctx, UINT32_C(3));",
        )?;
        self.line("spx_fused_row_receipt *spx_row_funding = NULL;");
        self.line(&format!(
            "spx_fused_rows_reserve(spx_domain, UINT64_C({count}), &spx_row_funding);"
        ));
        self.line(&format!("if (!spx_fused_rows_allocate_funded(spx_domain, spx_row_funding, UINT64_C({count}), &{rows})) {failure}"));
        self.apply(&allocation.id)?;
        let start = self.scalar(start, Kind::I64)?;
        let induction = self.bind(counter, Kind::I64)?;
        self.line(&format!("{induction} = {start};"));
        self.open("for (;;) {");
        let test = self.scalar(condition, Kind::Bool)?;
        self.line(&format!("if (!({test})) break;"));
        self.construction(body, &rows, &induction)?;
        self.close("}");
        self.line(&format!("if (!spx_fused_complete(spx_domain)) {AUTHORITY}"));
        Ok(())
    }
    fn construction(&mut self, body: &E, rows: &str, induction: &str) -> Result<(), Diagnostic> {
        let sites = self.sites;
        let K::Block { statements, tail } = &body.kind else {
            return Err(error("selected loop body is not a block"));
        };
        let [S::Let {
            binding: item,
            mutable: false,
            value: constructor,
            ..
        }, S::Assign {
            binding: destination,
            field: None,
            value: push,
            ..
        }, S::Assign {
            binding: latch,
            field: None,
            value: step,
            ..
        }] = statements.as_slice()
        else {
            return Err(error("selected loop body drifted from the selection"));
        };
        if !std::ptr::eq(item, sites.item)
            || !std::ptr::eq(constructor, sites.constructor)
            || !std::ptr::eq(push, sites.push)
            || !std::ptr::eq(step, sites.step)
            || destination.id != sites.vector.id
            || latch.id != sites.counter.id
        {
            return Err(error(
                "selected loop sites are not the selection's borrowed nodes",
            ));
        }
        self.scopes.push(Vec::new());
        let staged = self.row_operands(item, constructor, rows)?;
        self.publish(push, rows, &staged, item)?;
        let next = self.scalar(step, Kind::I64)?;
        self.line(&format!("{induction} = {next};"));
        let _ = self.expression(tail)?;
        self.close_scope()?;
        self.place_region(body)?;
        self.apply(&body.id)
    }
    /// Operands in initializer (declaration) order; the String is an ordinary
    /// owned String with its ordinary materialization and drop.
    fn row_operands(
        &mut self,
        item: &ResolvedBinding,
        constructor: &E,
        rows: &str,
    ) -> Result<String, Diagnostic> {
        let sites = self.sites;
        let K::ConstructRecord { record, fields } = &constructor.kind else {
            return Err(error(
                "selected row constructor is not a record construction",
            ));
        };
        if *record != sites.record.id || item.ty != constructor.ty || item.ty != sites.item.ty {
            return Err(error("selected row constructor targets a foreign record"));
        }
        let crate::hir::ResolvedTypeDeclarationKind::Record { fields: declared } =
            &sites.record.kind
        else {
            return Err(error("selected record declaration is not a record"));
        };
        if fields.len() != 3
            || declared.len() != 3
            || fields
                .iter()
                .zip(declared)
                .any(|(value, field)| value.field != field.id)
        {
            return Err(error(
                "selected row initializers are not in declaration order",
            ));
        }
        let staged = self.bind(item, Kind::Staged)?;
        self.register(
            StorageId::Value(item.id.clone()),
            Physical::Staged(staged.clone()),
        )?;
        self.register(
            StorageId::Temporary(constructor.id.clone()),
            Physical::Staged(staged.clone()),
        )?;
        for (initializer, role) in fields.iter().zip(self.order) {
            let value = &initializer.value;
            match role {
                Role::Scalar => {
                    let scalar = self.scalar(value, Kind::I64)?;
                    self.line(&format!("{staged}.scalar = {scalar};"));
                }
                Role::Text => {
                    let K::String(text) = &value.kind else {
                        return Err(error("selected String operand is not a literal"));
                    };
                    self.register(
                        StorageId::Temporary(value.id.clone()),
                        Physical::Field(staged.clone(), Role::Text),
                    )?;
                    self.line(&format!(
                        "{staged}.string = spx_string_from_literal(\"{}\", UINT64_C({}));",
                        crate::codegen::native_emit::c_string(text),
                        text.len()
                    ));
                    self.apply(&value.id)?;
                }
                Role::Payload => {
                    if !std::ptr::eq(value, sites.producer) {
                        return Err(error("selected Bytes operand is not the proven producer"));
                    }
                    self.register(
                        StorageId::Temporary(value.id.clone()),
                        Physical::Field(staged.clone(), Role::Payload),
                    )?;
                    self.line(&format!(
                        "if (!spx_fused_issue(spx_domain, (size_t){rows}.len, &{staged}.bytes)) {AUTHORITY}"
                    ));
                    self.apply(&value.id)?;
                }
            }
        }
        self.apply(&constructor.id)?;
        Ok(staged)
    }
    /// `destination = vec_push<Row>(destination, item)`: argument staging, the
    /// ordinary full-Vec failure through its plan exit, then the atomic commit.
    fn publish(
        &mut self,
        push: &E,
        rows: &str,
        staged: &str,
        item: &ResolvedBinding,
    ) -> Result<(), Diagnostic> {
        let sites = self.sites;
        let K::Call {
            callee,
            instance: None,
            args,
            ..
        } = &push.kind
        else {
            return Err(error("selected publication is not vec_push"));
        };
        let [destination, row] = args.as_slice() else {
            return Err(error("selected vec_push has the wrong arity"));
        };
        let reads = |e: &E, id: &ValueId| {
            e.ownership == O::Own
                && matches!(&e.kind, K::Place(p) if &p.root == id && p.projections.is_empty())
        };
        if callee.as_str() != crate::vec_ops::PUSH_ID
            || !reads(destination, &sites.vector.id)
            || !reads(row, &item.id)
        {
            return Err(error(
                "selected vec_push does not move the held Vec and staged row",
            ));
        }
        for (index, argument, physical) in [
            (0u32, destination, Physical::Rows(rows.to_owned())),
            (1u32, row, Physical::Staged(staged.to_owned())),
        ] {
            self.register(
                StorageId::CallArgument {
                    call: push.id.clone(),
                    parameter_index: index,
                    value_expression: argument.id.clone(),
                },
                physical,
            )?;
            self.apply(&argument.id)?;
        }
        self.register(
            StorageId::Temporary(push.id.clone()),
            Physical::Rows(rows.to_owned()),
        )?;
        let failure = self.fail(
            &push.id,
            "spx_status = spx_vec_failure(spx_ctx, UINT32_C(1));",
        )?;
        self.line(&format!(
            "if (!spx_fused_rows_has_room(spx_domain, &{rows})) {failure}"
        ));
        self.line(&format!(
            "if (!spx_fused_rows_push(spx_domain, &{rows}, &{staged})) {AUTHORITY}"
        ));
        self.apply(&push.id)
    }
}
