use super::undo;
#[test]
fn complete_original_sources_and_compiled_owning_receivers_remain_exact() {
    {
        let mut current = include_str!("../../../../hir/function_value/funded.rs").to_owned();
        undo(
            &mut current,
            r#"        }
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        }
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__function_value__funded.rs.before"
            ),
            "src/hir/function_value/funded.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/funded.rs").to_owned();
        undo(
            &mut current,
            r#"        assert!(fresh.is_empty(), "a refused charge publishes nothing");
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert!(fresh.is_empty(), "a refused charge publishes nothing");
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__funded.rs.before"
            ),
            "src/hir/funded.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/ids.rs").to_owned();
        undo(
            &mut current,
            r#"mod metered_owner;
#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;

#[cfg(test)]
pub(crate) use metered_owner::general_matches as oracle_owner_matches_with_work;
"#,
            r#"mod metered_owner;
#[cfg(test)]
pub(crate) use metered_owner::matches as monomorphic_owner_matches_with_work;
"#,
        );
        assert_eq!(
            current,
            include_str!("../../../../../docs/evidence/opt810-live-oracle/src__hir__ids.rs.before"),
            "src/hir/ids.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/ids/metered_owner.rs").to_owned();
        undo(
            &mut current,
            r#"        }
    }
}

mod oracle;
pub(crate) use oracle::matches as general_matches;
"#,
            r#"        }
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__ids__metered_owner.rs.before"
            ),
            "src/hir/ids/metered_owner.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/lexical_exit.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    visit(expr, None, 0) == RETURN
}

#[cfg(test)]
pub(crate) use owner_funded::{owner_funded_with_work, requires_funded_with_work};
"#,
            r#"    }
    visit(expr, None, 0) == RETURN
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__lexical_exit.rs.before"
            ),
            "src/hir/lexical_exit.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/lexical_exit/owner_funded.rs").to_owned();
        undo(
            &mut current,
            r#"        );
    }
}

mod work;
pub(crate) use work::{owner_funded_with_work, requires_funded_with_work};
"#,
            r#"        );
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__lexical_exit__owner_funded.rs.before"
            ),
            "src/hir/lexical_exit/owner_funded.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/lexical_exit/walk.rs").to_owned();
        undo(
            &mut current,
            r#"
#[cfg(test)]
mod tests;

impl<'a> ChildCursor<'a> {
    #[cfg(test)]
    pub(crate) fn next_with_work<E>(
        &mut self,
        work: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<&'a ResolvedExpr>, E> {
        work(1)?;
        let first = !self.started;
        self.started = true;
        Ok(match (&self.expression.kind, &mut self.position) {
            (
                K::Block { statements, tail }
                | K::VecConstructor {
                    statements, tail, ..
                },
                Position::Block {
                    statement,
                    child: index,
                },
            ) => {
                loop {
                    work(1)?;
                    let Some(item) = statements.get(*statement) else {
                        break;
                    };
                    if *index < item.child_count() {
                        let result = item.child(*index);
                        *index += 1;
                        return Ok(result);
                    }
                    *statement += 1;
                    *index = 0;
                }
                if *index == 0 {
                    *index = 1;
                    Some(tail)
                } else {
                    None
                }
            }
            (
                K::Match {
                    scrutinee, arms, ..
                },
                Position::Match { arm, guard },
            ) => {
                if first {
                    return Ok(Some(scrutinee));
                }
                if let Some(item) = arms.get(*arm) {
                    if *guard {
                        *guard = false;
                        if let Some(value) = &item.guard {
                            return Ok(Some(value));
                        }
                    }
                    *arm += 1;
                    *guard = true;
                    return Ok(Some(&item.value));
                }
                None
            }
            (_, Position::Indexed(index)) => {
                let result = child(self.expression, *index);
                if result.is_some() {
                    *index += 1;
                }
                result
            }
            _ => unreachable!("child cursor position matches its immutable expression"),
        })
    }
}
"#,
            r#"
#[cfg(test)]
mod tests;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__lexical_exit__walk.rs.before"
            ),
            "src/hir/lexical_exit/walk.rs"
        );
    }
    {
        let mut current = include_str!("../../../../literal_format.rs").to_owned();
        undo(
            &mut current,
            r#"        }
    }
}

#[cfg(test)]
pub(crate) use funded::work::checked_moving_string_operands_funded_with_work;
"#,
            r#"        }
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__literal_format.rs.before"
            ),
            "src/literal_format.rs"
        );
    }
    {
        let mut current = include_str!("../../../../literal_format/funded.rs").to_owned();
        undo(
            &mut current,
            r#"        assert_eq!(active, 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert_eq!(active, 0);
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__literal_format__funded.rs.before"
            ),
            "src/literal_format/funded.rs"
        );
    }
    {
        let mut current =
            include_str!("../../../../project/incremental/cold_source_tests.rs").to_owned();
        undo(
            &mut current,
            r#"    );
    same(&changed_warm, &old_changed);
}

mod oracle_owner_before;
"#,
            r#"    );
    same(&changed_warm, &old_changed);
}
"#,
        );
        undo(
            &mut current,
            r#"
#[test]
fn explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse() {
    crate::cleanup_plan::with_financed_test_oracles(
        oracle_owner_before::explicit_owner_siblings_match_legacy_selective_routes_and_body_reuse,
    );
}

#[test]
"#,
            &format!(
                "{}{}{}",
                r#"
#[test]
"#,
                include_str!(
                    "../../../../project/incremental/cold_source_tests/oracle_owner_before.rs"
                )
                .strip_prefix("use super::*;\n\npub(super) ")
                .unwrap()
                .trim_end_matches('\n'),
                r#"

#[test]
"#
            ),
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__project__incremental__cold_source_tests.rs.before"
            ),
            "src/project/incremental/cold_source_tests.rs"
        );
    }
    {
        let mut current = include_str!("../../../../string_ops.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    false
}

#[cfg(test)]
pub(crate) use concat_funded::work::{
    is_same_owner_concat_hir_funded_with_work, same_owner_concat_appends_funded_with_work,
    same_owner_concat_operands_funded_with_work,
};

#[cfg(test)]
pub(crate) fn by_id_with_work<E>(
    id: &str,
    work: crate::hir::funded::work::Work<'_, E>,
) -> Result<Option<StringOp>, crate::hir::funded::FundedError<E>> {
    use crate::hir::funded::work as paid;
    const KEYS: &[(&str, StringOp)] = &[
        (take::ID, StringOp::Take),
        (LEN_ID, StringOp::Len),
        (CONCAT_ID, StringOp::Concat),
        (IS_EMPTY_ID, StringOp::IsEmpty),
        (STARTS_WITH_ID, StringOp::StartsWith),
        (CONTAINS_ID, StringOp::Contains),
        (LEN_CHARS_ID, StringOp::LenChars),
        (FROM_CHAR_ID, StringOp::FromChar),
        (FROM_I64_ID, StringOp::FromI64),
        (FROM_USIZE_ID, StringOp::FromUsize),
        (SLICE_ID, StringOp::Slice),
        (FIND_ID, StringOp::Find),
        (TO_I64_ID, StringOp::ToI64),
        (TRIM_ID, StringOp::Trim),
        (BYTE_AT_ID, StringOp::ByteAt),
        (FILE_READ_TEXT_ID, StringOp::FileReadText),
        (COMPARE_ID, StringOp::Compare),
        (MAP_NEW_ID, StringOp::MapNew),
        (MAP_ADD_ID, StringOp::MapAdd),
        (MAP_SET_ID, StringOp::MapSet),
        (MAP_GET_OR_ID, StringOp::MapGetOr),
        (MAP_HAS_ID, StringOp::MapHas),
        (MAP_LEN_ID, StringOp::MapLen),
        (MAP_KEY_AT_ID, StringOp::MapKeyAt),
        (MAP_VALUE_AT_ID, StringOp::MapValueAt),
        (MAP_REMOVE_ID, StringOp::MapRemove),
        (FROM_STR_ID, StringOp::FromStr),
        (FROM_UTF8_ID, StringOp::FromUtf8),
        (F64_FROM_I64_ID, StringOp::F64FromI64),
        (I64_FROM_F64_ID, StringOp::I64FromF64),
        (USIZE_FROM_I64_ID, StringOp::UsizeFromI64),
        (I64_FROM_U8_ID, StringOp::I64FromU8),
        (I64_FROM_I32_ID, StringOp::I64FromI32),
        (USIZE_FROM_U8_ID, StringOp::UsizeFromU8),
        (U8_FROM_I64_ID, StringOp::U8FromI64),
        (CHAR_FROM_U8_ID, StringOp::CharFromU8),
        (CHAR_FROM_I64_ID, StringOp::CharFromI64),
        (I64_FROM_USIZE_ID, StringOp::I64FromUsize),
    ];
    for (key, op) in KEYS {
        paid::tick(work, paid::str_units(key, id))?;
        if *key == id {
            return Ok(Some(*op));
        }
    }
    Ok(None)
}
"#,
            r#"    }
    false
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops.rs.before"
            ),
            "src/string_ops.rs"
        );
    }
    {
        let mut current = include_str!("../../../../string_ops/concat_funded.rs").to_owned();
        undo(
            &mut current,
            r#"        assert!(value && calls > 1 && charged > 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert!(value && calls > 1 && charged > 0);
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__concat_funded.rs.before"
            ),
            "src/string_ops/concat_funded.rs"
        );
    }
    {
        let mut current = include_str!("../../../../string_ops/conditions.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    false
}

#[cfg(test)]
pub(crate) use funded::work::{
    condition_reads_funded_with_work, function_reads_funded_with_work,
    needs_cleanup_funded_with_work,
};

#[cfg(test)]
pub(crate) fn resolved_operands_with_work<'a, E>(
    expression: &'a ResolvedExpr,
    work: crate::hir::funded::work::Work<'_, E>,
) -> Result<Option<&'a [ResolvedExpr]>, crate::hir::funded::FundedError<E>> {
    use crate::hir::funded::work as paid;
    paid::tick(work, 1)?;
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    let Some(op) = super::by_id_with_work(callee.as_str(), work)? else {
        return Ok(None);
    };
    paid::tick(work, 7)?;
    if !is_condition_read(op)
        || !type_arguments.is_empty()
        || instance.is_some()
        || expression.ty != op.return_type()
        || expression.ownership != OwnershipMode::Value
        || args.len() != op.arity()
    {
        return Ok(None);
    }
    for (index, argument) in args.iter().enumerate() {
        paid::tick(work, 5)?;
        if op.param_ownership(index) != OwnershipMode::Borrow
            || argument.ty != ResolvedType::String
            || argument.ownership != OwnershipMode::Own
            || !matches!(&argument.kind,ResolvedExprKind::Place(place) if place.projections.is_empty())
        {
            return Ok(None);
        }
    }
    Ok(Some(args))
}
"#,
            r#"    }
    false
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__conditions.rs.before"
            ),
            "src/string_ops/conditions.rs"
        );
    }
    {
        let mut current = include_str!("../../../../string_ops/conditions/funded.rs").to_owned();
        undo(
            &mut current,
            r#"        assert_eq!(active, 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
"#,
            r#"        assert_eq!(active, 0);
    }
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__conditions__funded.rs.before"
            ),
            "src/string_ops/conditions/funded.rs"
        );
    }
    {
        let mut current = include_str!("../../../../string_ops/take.rs").to_owned();
        undo(
            &mut current,
            r#"
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn operand_with_work<'a, E>(
    expression: &'a ResolvedExpr,
    work: crate::hir::funded::work::Work<'_, E>,
) -> Result<Option<&'a ResolvedExpr>, crate::hir::funded::FundedError<E>> {
    use crate::hir::funded::work as paid;
    paid::tick(work, 1)?;
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    paid::tick(work, paid::str_units(callee.as_str(), ID))?;
    if callee.as_str() != ID {
        return Ok(None);
    }
    // The complete original predicate inspects only these bounded scalar
    // discriminants/lengths. It performs no hidden lookup or tree walk.
    paid::tick(work, 10)?;
    if instance.is_some()
        || !type_arguments.is_empty()
        || expression.ty != ResolvedType::String
        || expression.ownership != OwnershipMode::Own
    {
        return Ok(None);
    }
    let [argument] = args.as_slice() else {
        return Ok(None);
    };
    Ok((argument.ty == ResolvedType::String
        && argument.ownership == OwnershipMode::Own
        && matches!(&argument.kind,ResolvedExprKind::Place(place) if place.projections.is_empty()))
    .then_some(argument))
}
"#,
            r#"
#[cfg(test)]
mod tests;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__string_ops__take.rs.before"
            ),
            "src/string_ops/take.rs"
        );
    }
    {
        let mut current =
            include_str!("../../../../workspace_graph/identity_resolution/tests.rs").to_owned();
        undo(
            &mut current,
            r#"        assert_eq!(actual.1.costs(), expected_costs);
    }
}

mod oracle_owner_before;
"#,
            r#"        assert_eq!(actual.1.costs(), expected_costs);
    }
}
"#,
        );
        undo(
            &mut current,
            r#"/// Both results remain live until Drop inside the allocation observation.
#[test]
fn one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries() {
    crate::cleanup_plan::with_financed_test_oracles(
        oracle_owner_before::one_mib_ledger_preserves_first_cursor_refusal_and_sticky_retries,
    );
}

#[test]
"#,
            &format!(
                "{}{}{}",
                r#"/// Both results remain live until Drop inside the allocation observation.
#[test]
"#,
                include_str!(
                    "../../../../workspace_graph/identity_resolution/tests/oracle_owner_before.rs"
                )
                .strip_prefix("use super::*;\n\npub(super) ")
                .unwrap()
                .trim_end_matches('\n'),
                r#"

#[test]
"#
            ),
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__workspace_graph__identity_resolution__tests.rs.before"
            ),
            "src/workspace_graph/identity_resolution/tests.rs"
        );
    }
    {
        let mut current = include_str!("../../../../../tests/hir_module_boundaries.rs").to_owned();
        undo(&mut current, r###"        include_str!("../src/cleanup_plan/replay/factored/oracle_hook/closure_controls.rs"),
"###, "");
        undo(
            &mut current,
            r#"        include_str!("../src/hir/iterator_loop/renewal/paid_scan.rs"),
        include_str!("../src/hir/iterator_loop/renewal/funded.rs"),
        include_str!("../src/hir/funded.rs"),
        include_str!("../src/bounded_output/classifier_string/oracle_work.rs"),
        include_str!("../src/cleanup_plan/replay/allocation/oracle/diagnostic/metered.rs"),
        include_str!("../src/cleanup_plan/replay/factored/oracle_order.rs"),
        include_str!("../src/cleanup_plan/replay/factored/oracle_order/tests.rs"),
        include_str!("../src/cleanup_plan/replay/live_oracle.rs"),
        include_str!("../src/cleanup_plan/replay/nested_shape/paid/oracle.rs"),
        include_str!("../src/cleanup_plan/replay/paid_type_facts/oracle_inventory.rs"),
        include_str!("../src/cleanup_plan/replay/paid_type_facts/oracle_inventory/tests.rs"),
        include_str!("../src/hir/function_value/funded/work.rs"),
        include_str!("../src/hir/funded/work.rs"),
        include_str!("../src/hir/ids/metered_owner/oracle.rs"),
        include_str!("../src/hir/lexical_exit/owner_funded/work.rs"),
        include_str!("../src/literal_format/funded/work.rs"),
        include_str!("../src/project/incremental/cold_source_tests/oracle_owner_before.rs"),
        include_str!("../src/string_ops/concat_funded/work.rs"),
        include_str!("../src/string_ops/conditions/funded/work.rs"),
        include_str!("../src/workspace_graph/identity_resolution/tests/oracle_owner_before.rs"),

        include_str!("../src/hir/ordinary_scan.rs"),
        include_str!("../src/hir/ordinary_scan/tests.rs"),
        include_str!("../src/hir/function_value/funded.rs"),
"#,
            r#"        include_str!("../src/hir/iterator_loop/renewal/paid_scan.rs"),
        include_str!("../src/hir/iterator_loop/renewal/funded.rs"),
        include_str!("../src/hir/funded.rs"),
        include_str!("../src/hir/ordinary_scan.rs"),
        include_str!("../src/hir/ordinary_scan/tests.rs"),
        include_str!("../src/hir/function_value/funded.rs"),
"#,
        );
        undo(
            &mut current,
            r#"        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_summary/shape_original.rs"),
"#,
            "",
        );
        undo(
            &mut current,
            r#"        include_str!("../src/cleanup_plan/replay/paid_type_facts/inventory_summary/frontier_tests.rs"),
"#,
            "",
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/tests__hir_module_boundaries.rs.before"
            ),
            "tests/hir_module_boundaries.rs"
        );
    }
}
