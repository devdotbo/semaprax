// Saved original iterative validator arms, preserving literal content.
macro_rules! owned_dispatch_2 {
    ($frame:expr, $this:ident, $function:ident, $allow_moves:ident, $allowed_effects:ident, $program:ident, $publication:ident, $frames:ident, $scopes:ident, $Frame:ident) => {
        match $frame {
                $Frame::RecordAfterField {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    path,
                } => {
                    let mut scope = $scopes.pop().expect("record field scope retained");
                    $publication.publish(&scope);
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    $this.require_type(&initializer.value.ty, &field_ty, "record field")?;
                    let ownership = $this.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(hir_error(format!(
                            "field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !$allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record",
                            ));
                        }
                        $this.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        $publication.publish(&scope);
                    }
                    $frames.push($Frame::RecordNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                $Frame::VariantNext {
                    expression,
                    fields,
                    expected,
                    variant,
                    case,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                } => {
                    if index == fields.len() {
                        if seen.len() != expected.len() {
                            return Err(hir_error(format!(
                                "constructor for `{case}` is missing required payload fields"
                            )));
                        }
                        let ownership =
                            $this.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                        $this.finish_expr(expression, &expression.ty, ownership)?;
                        $scopes.push(scope);
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(hir_error(format!(
                                "constructor for `{case}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(hir_error(format!(
                                "constructor for `{case}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        $frames.push($Frame::VariantAfterField {
                            expression,
                            fields,
                            expected,
                            variant,
                            case,
                            arguments,
                            seen,
                            index,
                            path,
                        });
                        $frames.push($Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                $Frame::VariantAfterField {
                    expression,
                    fields,
                    expected,
                    variant,
                    case,
                    arguments,
                    seen,
                    index,
                    path,
                } => {
                    let mut scope = $scopes.pop().expect("variant field scope retained");
                    $publication.publish(&scope);
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("variant field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, variant, arguments)?;
                    $this.require_type(&initializer.value.ty, &field_ty, "variant payload field")?;
                    let ownership = $this.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != ownership {
                        return Err(hir_error(format!(
                            "variant payload field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !$allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a variant",
                            ));
                        }
                        $this.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        $publication.publish(&scope);
                    }
                    $frames.push($Frame::VariantNext {
                        expression,
                        fields,
                        expected,
                        variant,
                        case,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                    });
                }
                $Frame::UpdateBase {
                    expression,
                    record,
                    fields,
                    path,
                } => {
                    let mut scope = $scopes.pop().expect("update base scope retained");
                    $publication.publish(&scope);
                    let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let declaration = $this
                        .$program
                        .declarations
                        .declaration(record)
                        .ok_or_else(|| hir_error(format!("record `{record}` is not indexed")))?;
                    if declaration.kind != DeclarationKind::Record {
                        return Err(hir_error(format!(
                            "record update target `{record}` is not a record"
                        )));
                    }
                    let ResolvedType::Nominal {
                        declaration: instance,
                        arguments,
                    } = &base.ty
                    else {
                        return Err(hir_error("record update base is not nominal"));
                    };
                    let parameters = $this
                        .$program
                        .declarations
                        .type_parameters(record)
                        .ok_or_else(|| hir_error(format!("record `{record}` has no parameters")))?;
                    if instance != record
                        || arguments.len() != parameters.len()
                        || !super::type_reachability::record_args_ok(
                            &$this.$program.declarations,
                            record,
                            arguments,
                        )
                    {
                        return Err(hir_error(format!(
                            "record update for `{record}` has an invalid concrete instance"
                        )));
                    }
                    validate_nested_update_base_shape($this.$program, base, expression.span)?;
                    let ownership = $this.expected_ownership(&base.ty, OwnershipMode::Own)?;
                    if base.ownership != ownership {
                        return Err(hir_error(format!(
                            "record update base for `{record}` has incompatible ownership"
                        )));
                    }
                    if ownership == OwnershipMode::Own {
                        if !$allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership from a record update base",
                            ));
                        }
                        $this.mark_value_sources_moved(base, &mut scope)?;
                        $publication.publish(&scope);
                    }
                    let expected = aggregate_metadata::record_fields($program, record)
                        .ok_or_else(|| hir_error(format!("record `{record}` has no fields")))?;
                    $frames.push($Frame::UpdateNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen: BTreeSet::new(),
                        index: 0,
                        scope,
                        path,
                        ownership,
                    });
                }
                $Frame::UpdateNext {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    scope,
                    path,
                    ownership,
                } => {
                    if index == fields.len() {
                        let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
                            unreachable!()
                        };
                        $this.finish_expr(expression, &base.ty, ownership)?;
                        $scopes.push(scope);
                    } else {
                        let initializer = &fields[index];
                        let mut seen = seen;
                        if !expected.iter().any(|field| field.id == initializer.field) {
                            return Err(hir_error(format!(
                                "update for `{record}` contains foreign field `{}`",
                                initializer.field
                            )));
                        }
                        if !seen.insert(initializer.field.clone()) {
                            return Err(hir_error(format!(
                                "update for `{record}` repeats field `{}`",
                                initializer.field
                            )));
                        }
                        $frames.push($Frame::UpdateAfterField {
                            expression,
                            fields,
                            expected,
                            record,
                            arguments,
                            seen,
                            index,
                            path,
                            ownership,
                        });
                        $frames.push($Frame::Enter {
                            expression: &initializer.value,
                            scope,
                            path: format!("{path}.field.{index}.value"),
                        });
                    }
                }
                $Frame::UpdateAfterField {
                    expression,
                    fields,
                    expected,
                    record,
                    arguments,
                    seen,
                    index,
                    path,
                    ownership,
                } => {
                    let mut scope = $scopes.pop().expect("update field scope retained");
                    $publication.publish(&scope);
                    let initializer = &fields[index];
                    let declared = expected
                        .iter()
                        .find(|field| field.id == initializer.field)
                        .expect("update field authenticated before child");
                    let field_ty = substitute_type(&declared.ty, record, arguments)?;
                    $this.require_type(&initializer.value.ty, &field_ty, "record replacement")?;
                    let expected_ownership =
                        $this.expected_ownership(&field_ty, OwnershipMode::Own)?;
                    if initializer.value.ownership != expected_ownership {
                        return Err(hir_error(format!(
                            "replacement field `{}` has incompatible ownership",
                            initializer.field
                        )));
                    }
                    if expected_ownership == OwnershipMode::Own {
                        if !$allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a record replacement",
                            ));
                        }
                        $this.mark_value_sources_moved(&initializer.value, &mut scope)?;
                        $publication.publish(&scope);
                    }
                    $frames.push($Frame::UpdateNext {
                        expression,
                        fields,
                        expected,
                        record,
                        arguments,
                        seen,
                        index: index + 1,
                        scope,
                        path,
                        ownership,
                    });
                }
                $Frame::MatchScrutinee {
                    expression,
                    arms,
                    path,
                } => {
                    let mut outer = $scopes.pop().expect("match scrutinee scope retained");
                    $publication.publish(&outer);
                    let ResolvedExprKind::Match {
                        mode, scrutinee, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    // Refutable Match v1: Copy-scalar scrutinees validate
                    // through the decision-chain twins below.
                    if matches!(
                        scrutinee.ty,
                        ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::U8
                            | ResolvedType::Usize
                            | ResolvedType::Char
                            | ResolvedType::Bool
                    ) {
                        if *mode != ResolvedMatchMode::Value
                            || scrutinee.ownership != OwnershipMode::Value
                        {
                            return Err(hir_error(
                                "resolved refutable match scrutinee is not Copy",
                            ));
                        }
                        if arms.is_empty() {
                            return Err(hir_error("resolved match has no arms"));
                        }
                        let outer_ids = outer.keys().cloned().collect::<Vec<_>>();
                        $frames.push($Frame::ScalarMatchNext {
                            expression,
                            arms,
                            index: 0,
                            outer,
                            outer_ids,
                            arm_scopes: Vec::with_capacity(arms.len()),
                            result: None,
                            path,
                        });
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration: matched,
                        arguments,
                    } = &scrutinee.ty
                    else {
                        return Err(hir_error("resolved match scrutinee is not nominal"));
                    };
                    let kind = $this
                        .$program
                        .declarations
                        .declaration(matched)
                        .map(|item| item.kind);
                    let outer_ids = outer.keys().cloned().collect::<Vec<_>>();
                    if kind == Some(DeclarationKind::Record) {
                        let facts = $this
                            .$program
                            .declarations
                            .type_facts(&scrutinee.ty)
                            .ok_or_else(|| hir_error("record match has no exact type facts"))?;
                        match mode {
                            ResolvedMatchMode::Value
                                if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                            ResolvedMatchMode::Own
                                if facts.needs_drop
                                    && !facts.copy
                                    && scrutinee.ownership == OwnershipMode::Own
                                    && (!super::type_reachability::is_nested_nonflat_owned_byte_record(
                                        &$this.$program.declarations,
                                        &scrutinee.ty,
                                    ) || matches!(
                                        &scrutinee.kind,
                                        ResolvedExprKind::Place(place)
                                            if place.projections.is_empty()
                                    )) =>
                            {
                                $this.mark_value_sources_moved(scrutinee, &mut outer)?;
                            }
                            ResolvedMatchMode::Borrow
                                if facts.needs_drop
                                    && !facts.copy
                                    && matches!(
                                        scrutinee.ownership,
                                        OwnershipMode::Own | OwnershipMode::Borrow
                                    )
                                    && matches!(
                                        &scrutinee.kind,
                                        ResolvedExprKind::Place(place)
                                            if place.projections.is_empty()
                                    ) => {}
                            _ => {
                                return Err(hir_error(
                                    "resolved record match mode disagrees with its scrutinee",
                                ));
                            }
                        }
                        let [arm] = arms else {
                            return Err(hir_error(
                                "resolved irrefutable record match must have exactly one arm",
                            ));
                        };
                        let mut arm_scope = outer.clone();
                        let borrowed_root = if *mode == ResolvedMatchMode::Borrow {
                            let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                                unreachable!("borrow mode authenticated a place above")
                            };
                            if let Some(owner) = arm_scope.get_mut(&place.root) {
                                owner.active_loans.extend($this.exact_loan_id(
                                    $function,
                                    &expression.id,
                                    &LoanCause::MatchBorrow { arm: 0 },
                                )?);
                            }
                            Some(place.root.clone())
                        } else {
                            None
                        };
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard if *mode != ResolvedMatchMode::Value => {
                                return Err(hir_error(
                                    "resolved explicit ownership match lacks an exact record pattern",
                                ));
                            }
                            ResolvedMatchPattern::Wildcard => {}
                            ResolvedMatchPattern::Record {
                                record,
                                instance,
                                fields,
                            } => $this
                                .validate_record_match_pattern_owned_reference(
                                    $function,
                                    &scrutinee.ty,
                                    record,
                                    instance,
                                    fields,
                                    &mut arm_scope,
                                    &format!("{path}.arm.0.record"),
                                    *mode,
                                )
                                .map_err(|diagnostic| {
                                    hir_diagnostic_at_span(diagnostic, expression.span)
                                })?,
                            ResolvedMatchPattern::Variant { .. } => {
                                return Err(hir_error(
                                    "resolved variant pattern has a record scrutinee",
                                ));
                            }
                            ResolvedMatchPattern::Literal(_)
                            | ResolvedMatchPattern::Or(_)
                            | ResolvedMatchPattern::Binding(_) => {
                                return Err(hir_error(
                                    "resolved refutable pattern has an aggregate record \
                                     scrutinee",
                                ));
                            }
                        }
                        $frames.push($Frame::RecordMatchArm {
                            expression,
                            arm,
                            outer,
                            outer_ids,
                            borrowed_root,
                        });
                        let enabled = $publication.enabled;
                        $publication.enabled = false;
                        $frames.push($Frame::RestorePublication(enabled));
                        $frames.push($Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: format!("{path}.arm.0.value"),
                        });
                    } else {
                        if kind != Some(DeclarationKind::Variant) {
                            return Err(hir_error(
                                "resolved match scrutinee is not a concrete variant",
                            ));
                        }
                        let facts = $this
                            .$program
                            .declarations
                            .type_facts(&scrutinee.ty)
                            .ok_or_else(|| hir_error("variant match has no exact type facts"))?;
                        match mode {
                            ResolvedMatchMode::Value
                                if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                            ResolvedMatchMode::Own
                                if (resolved_type_is_flat_owned_byte_variant(
                                    $this.$program,
                                    &scrutinee.ty,
                                ) || resolved_type_is_direct_owned_variant(
                                    $this.$program,
                                    &scrutinee.ty,
                                )) && facts.needs_drop
                                    && !facts.copy
                                    && scrutinee.ownership == OwnershipMode::Own =>
                            {
                                $this.mark_value_sources_moved(scrutinee, &mut outer)?;
                            }
                            ResolvedMatchMode::Borrow
                                if (resolved_type_is_flat_owned_byte_variant(
                                    $this.$program,
                                    &scrutinee.ty,
                                ) || resolved_type_is_direct_owned_variant(
                                    $this.$program,
                                    &scrutinee.ty,
                                ) || regex_result($this.$program, &scrutinee.ty))
                                    && facts.needs_drop
                                    && !facts.copy
                                    && matches!(
                                        scrutinee.ownership,
                                        OwnershipMode::Own | OwnershipMode::Borrow
                                    )
                                    && matches!(&scrutinee.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()) =>
                                {}
                            _ => {
                                return Err(hir_error(
                                    "resolved variant match mode disagrees with its scrutinee",
                                ));
                            }
                        }
                        let cases = aggregate_metadata::variant_cases($program, matched)
                            .ok_or_else(|| {
                                hir_error(format!("variant `{matched}` has no cases"))
                            })?;
                        if arms.is_empty() {
                            return Err(hir_error("resolved match has no arms"));
                        }
                        $frames.push($Frame::VariantMatchNext {
                            expression,
                            arms,
                            cases,
                            variant: matched,
                            arguments,
                            index: 0,
                            outer,
                            outer_ids,
                            arm_scopes: Vec::with_capacity(arms.len()),
                            covered: BTreeSet::new(),
                            wildcard_seen: false,
                            result: None,
                            path,
                        });
                    }
                }
                $Frame::RecordMatchArm {
                    expression,
                    arm,
                    mut outer,
                    outer_ids,
                    borrowed_root,
                } => {
                    let mut arm_scope = $scopes.pop().expect("record match arm scope retained");
                    if let Some(root) = borrowed_root {
                        if let Some(owner) = arm_scope.get_mut(&root) {
                            if let Some(loan) = $this.exact_loan_id(
                                $function,
                                &expression.id,
                                &LoanCause::MatchBorrow { arm: 0 },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    $this.validate_generic_record_match_result($function, expression, arm)?;
                    for id in outer_ids {
                        if let Some(state) = arm_scope.get(&id) {
                            outer.insert(id, state.clone());
                        }
                    }
                    $publication.publish(&outer);
                    $this.finish_expr(expression, &arm.value.ty, arm.value.ownership)?;
                    $scopes.push(outer);
                }
                $Frame::VariantMatchNext {
                    expression,
                    arms,
                    cases,
                    variant,
                    arguments,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    mut covered,
                    mut wildcard_seen,
                    result,
                    path,
                } => {
                    let ResolvedExprKind::Match {
                        scrutinee, mode, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    if index == arms.len() {
                        if !wildcard_seen
                            && covered.len() != cases.len()
                            && !$this.observed_partial_byte_receiver(expression)
                        {
                            return Err(hir_error("resolved match is not exhaustive"));
                        }
                        let (ty, ownership) =
                            result.ok_or_else(|| hir_error("resolved match has no result"))?;
                        let mut final_scope = outer;
                        if let Some((first, rest)) = arm_scopes.split_first() {
                            let mut joined = first.clone();
                            for arm_scope in rest {
                                Self::join_conditional(&mut joined, arm_scope, &outer_ids);
                            }
                            Self::merge_availability(&mut final_scope, &joined, &outer_ids);
                            Self::merge_lexical_borrows(&mut final_scope, &joined, &outer_ids);
                        }
                        $publication.publish(&final_scope);
                        $this.finish_expr(expression, &ty, ownership)?;
                        $scopes.push(final_scope);
                    } else {
                        let arm = &arms[index];
                        let mut arm_scope = outer.clone();
                        if *mode == ResolvedMatchMode::Borrow {
                            let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                                unreachable!()
                            };
                            if let Some(owner) = arm_scope.get_mut(&place.root) {
                                owner.active_loans.extend($this.exact_loan_id(
                                    $function,
                                    &expression.id,
                                    &LoanCause::MatchBorrow {
                                        arm: u16::try_from(index).map_err(|_| {
                                            hir_error("match loan arm index overflows")
                                        })?,
                                    },
                                )?);
                            }
                        }
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard => {
                                if *mode != ResolvedMatchMode::Value {
                                    return Err(hir_error(
                                        "resolved explicit ownership match cannot discard an owned variant",
                                    ));
                                }
                                if wildcard_seen || covered.len() == cases.len() {
                                    return Err(hir_error(
                                        "resolved match has an unreachable wildcard",
                                    ));
                                }
                                wildcard_seen = arm.guard.is_none();
                            }
                            ResolvedMatchPattern::Variant {
                                variant: pattern_variant,
                                case,
                                fields,
                            } => {
                                if wildcard_seen
                                    || pattern_variant != variant
                                    || covered.contains(case)
                                    || (arm.guard.is_none() && !covered.insert(case.clone()))
                                {
                                    return Err(hir_error(
                                        "resolved match has an unreachable or foreign case pattern",
                                    ));
                                }
                                let declared_case = cases
                                    .iter()
                                    .find(|item| item.id == *case)
                                    .ok_or_else(|| {
                                        hir_error(format!(
                                            "resolved match references foreign case `{case}`"
                                        ))
                                    })?;
                                let mut seen = BTreeSet::new();
                                for (field_index, field) in fields.iter().enumerate() {
                                    let declared = declared_case
                                        .fields
                                        .iter()
                                        .find(|item| item.id == field.field)
                                        .ok_or_else(|| {
                                            hir_error(format!(
                                                "resolved pattern contains foreign field `{}`",
                                                field.field
                                            ))
                                        })?;
                                    let binding_ty =
                                        substitute_type(&declared.ty, variant, arguments)?;
                                    let facts = $this
                                        .$program
                                        .declarations
                                        .type_facts(&binding_ty)
                                        .ok_or_else(|| {
                                        hir_error("variant binding has no exact type facts")
                                    })?;
                                    let binding_ownership = if facts.copy {
                                        OwnershipMode::Value
                                    } else if *mode == ResolvedMatchMode::Own {
                                        OwnershipMode::Own
                                    } else if *mode == ResolvedMatchMode::Borrow {
                                        OwnershipMode::Borrow
                                    } else {
                                        return Err(hir_error(
                                            "Copy variant pattern contains an owned payload",
                                        ));
                                    };
                                    if !seen.insert(field.field.clone())
                                        || !field.binding.id.matches_local(
                                            $function,
                                            &format!("{path}.arm.{index}.binding.{field_index}"),
                                        )
                                        || field.binding.ty != binding_ty
                                        || field.binding.ownership != binding_ownership
                                    {
                                        return Err(hir_error(
                                            "resolved match pattern field or binding is invalid",
                                        ));
                                    }
                                    $this.insert_value(&field.binding.id)?;
                                    $this.validate_type(&field.binding.ty)?;
                                    if arm_scope.contains_key(&field.binding.id) {
                                        return Err(hir_error(
                                            "resolved match pattern binding shadows an existing value",
                                        ));
                                    }
                                    arm_scope.insert(
                                        field.binding.id.clone(),
                                        ValidationBinding {
                                            contract_local: false,
                                            mutable: false,
                                            ty: field.binding.ty.clone(),
                                            ownership: binding_ownership,
                                            availability: Availability::Available,
                                            active_loans: BTreeSet::new(),
                                            moved_places: BTreeMap::new(),
                                            definitely_partial: BTreeSet::new(),
                                        },
                                    );
                                }
                                if seen.len() != declared_case.fields.len() {
                                    return Err(hir_error(
                                        "resolved match pattern is missing payload fields",
                                    ));
                                }
                            }
                            ResolvedMatchPattern::Record { .. } => {
                                return Err(hir_error(
                                    "resolved record pattern has a variant scrutinee",
                                ));
                            }
                            ResolvedMatchPattern::Or(alternatives) => {
                                let mut candidate = covered.clone();
                                variant_or::cover_variant_or_pattern(
                                    alternatives,
                                    variant,
                                    cases,
                                    *mode,
                                    wildcard_seen,
                                    &mut candidate,
                                )?;
                                if arm.guard.is_none() {
                                    covered = candidate;
                                }
                            }
                            ResolvedMatchPattern::Literal(_) | ResolvedMatchPattern::Binding(_) => {
                                return Err(hir_error(
                                    "resolved refutable pattern has an aggregate variant \
                                     scrutinee",
                                ));
                            }
                        }
                        if let Some(guard) = &arm.guard {
                            if !crate::variant_guards::admitted(
                                &$this.$program.declarations,
                                &scrutinee.ty,
                                *mode,
                                &arm.pattern,
                                guard,
                            ) {
                                return Err(hir_error(
                                    "resolved Copy variant guard is outside its ordinary Boolean profile",
                                ));
                            }
                            $this.validate_expr_iterative_owned_reference(
                                $function,
                                guard,
                                &mut arm_scope,
                                &format!("{path}.arm.{index}.guard"),
                                $allow_moves,
                                $allowed_effects,
                            )?;
                            if guard.ty != ResolvedType::Bool {
                                return Err(hir_error("resolved match guard is not bool"));
                            }
                            variant_guard_state::unchanged(&outer, &arm_scope)?;
                        }
                        $frames.push($Frame::VariantMatchAfterArm {
                            expression,
                            arms,
                            cases,
                            variant,
                            arguments,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            covered,
                            wildcard_seen,
                            result,
                            path,
                        });
                        let enabled = $publication.enabled;
                        $publication.enabled = false;
                        $frames.push($Frame::RestorePublication(enabled));
                        $frames.push($Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: format!("{path}.arm.{index}.value"),
                        });
                    }
                }
                $Frame::VariantMatchAfterArm {
                    expression,
                    arms,
                    cases,
                    variant,
                    arguments,
                    index,
                    outer,
                    outer_ids,
                    mut arm_scopes,
                    covered,
                    wildcard_seen,
                    mut result,
                    path,
                } => {
                    let mut arm_scope = $scopes.pop().expect("variant match arm scope retained");
                    if let ResolvedExprKind::Match {
                        scrutinee,
                        mode: ResolvedMatchMode::Borrow,
                        ..
                    } = &expression.kind
                    {
                        let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                            unreachable!()
                        };
                        if let Some(owner) = arm_scope.get_mut(&place.root) {
                            if let Some(loan) = $this.exact_loan_id(
                                $function,
                                &expression.id,
                                &LoanCause::MatchBorrow {
                                    arm: u16::try_from(index)
                                        .map_err(|_| hir_error("match loan arm index overflows"))?,
                                },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    let arm = &arms[index];
                    if arm.value.ty != ResolvedType::Never
                        && !super::collection_outcome::match_arm_execution(
                            $this.$program,
                            $function,
                            expression,
                            &arm.value,
                        )
                    {
                        return Err(hir_error(
                            "resolved owned variant match arm must produce a Copy scalar",
                        ));
                    }
                    $this.join_lexical_match(&mut result, &arm.value)?;
                    if !crate::hir::lexical_exit::returns_only(&arm.value) {
                        arm_scopes.push(arm_scope);
                    }
                    $frames.push($Frame::VariantMatchNext {
                        expression,
                        arms,
                        cases,
                        variant,
                        arguments,
                        index: index + 1,
                        outer,
                        outer_ids,
                        arm_scopes,
                        covered,
                        wildcard_seen,
                        result,
                        path,
                    });
                }
            other => owned_dispatch_3!(other, $this, $function, $allow_moves, $allowed_effects, $program, $publication, $frames, $scopes, $Frame),
        }
    };
}
