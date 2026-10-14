use super::undo;
#[test]
fn complete_original_sources_and_compiled_owning_receivers_remain_exact() {
    {
        let mut current =
            include_str!("../../../../bounded_output/classifier_string.rs").to_owned();
        undo(
            &mut current,
            r#"mod explicit;

mod inventory_work;

#[cfg(test)]
mod oracle_work;
"#,
            r#"mod explicit;

mod inventory_work;
"#,
        );
        undo(
            &mut current,
            r#"/// cached admission or source authority. Its caller pays its containing control.
pub(crate) struct ClassifierLedger(Option<Rc<Budget>>);
impl ClassifierLedger {
    /// A test-account caller authenticates its own invocation separately. This
    /// detached marker contains no ACTIVE handle and grants no ordinary payer.
    #[cfg(test)]
    pub(crate) fn oracle_marker() -> Self {
        Self(None)
    }

    pub(crate) fn capture() -> Self {
        Self(active())
    }
"#,
            r#"/// cached admission or source authority. Its caller pays its containing control.
pub(crate) struct ClassifierLedger(Option<Rc<Budget>>);
impl ClassifierLedger {
    pub(crate) fn capture() -> Self {
        Self(active())
    }
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__bounded_output__classifier_string.rs.before"
            ),
            "src/bounded_output/classifier_string.rs"
        );
    }
    {
        let mut current = include_str!("../../../../cleanup_plan.rs").to_owned();
        undo(
            &mut current,
            r#"
// Shared pinned compiler container layout, not cleanup authority.
pub(crate) use replay::replay_btree_node_upper as compiler_btree_node_upper;

#[cfg(test)]
pub(crate) use replay::with_financed_test_oracles;
"#,
            r#"
// Shared pinned compiler container layout, not cleanup authority.
pub(crate) use replay::replay_btree_node_upper as compiler_btree_node_upper;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan.rs.before"
            ),
            "src/cleanup_plan.rs"
        );
    }
    {
        let mut current = include_str!("../../../replay.rs").to_owned();
        undo(&mut current, "pub(crate) use live_oracle::with_financed_test_oracles;", "pub(super) use live_oracle::with_financed_test_oracles;");
        undo(
            &mut current,
            r#"
#[cfg(test)]
pub(super) mod phase_measurement;

#[cfg(test)]
mod live_oracle;
#[cfg(test)]
pub(super) use live_oracle::with_financed_test_oracles;
"#,
            r#"
#[cfg(test)]
pub(super) mod phase_measurement;
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay.rs.before"
            ),
            "src/cleanup_plan/replay.rs"
        );
    }
    {
        let mut current = include_str!("../../allocation.rs").to_owned();
        undo(
            &mut current,
            r#"mod oracle;
#[cfg(test)]
pub(super) use oracle::{
    InvocationRefusal, OracleLease, OracleReceipt, OracleRefusal, OracleScope, ReplayFailure,
    authority, authority_for_function, enter, opt_in, opted, raw_error_funded, raw_error_metered,
    replay_error_funded,
};
pub(super) fn set_key<K: Ord + ClonePayload>(
    function: &ResolvedFunction,
"#,
            r#"mod oracle;
#[cfg(test)]
pub(super) use oracle::{
    authority, authority_for_function, enter, opt_in, opted, raw_error_funded, replay_error_funded,
    InvocationRefusal, OracleLease, OracleReceipt, OracleRefusal, OracleScope, ReplayFailure,
};
pub(super) fn set_key<K: Ord + ClonePayload>(
    function: &ResolvedFunction,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation.rs.before"
            ),
            "src/cleanup_plan/replay/allocation.rs"
        );
    }
    {
        let mut current = include_str!("../../allocation/oracle.rs").to_owned();
        undo(
            &mut current,
            r#"mod tests;

pub(in crate::cleanup_plan::replay) use diagnostic::{
    ReplayFailure, raw_error_funded, raw_error_metered, replay_error_funded,
};
pub(in crate::cleanup_plan::replay) use invocation::{
    authority, authority_for_function, enter, opt_in, opted, InvocationRefusal,
"#,
            r#"mod tests;

pub(in crate::cleanup_plan::replay) use diagnostic::{
    raw_error_funded, replay_error_funded, ReplayFailure,
};
pub(in crate::cleanup_plan::replay) use invocation::{
    authority, authority_for_function, enter, opt_in, opted, InvocationRefusal,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation__oracle.rs.before"
            ),
            "src/cleanup_plan/replay/allocation/oracle.rs"
        );
    }
    {
        let mut current = include_str!("../../allocation/oracle/diagnostic.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    Ok(Diagnostic::io("SPX-H006", text))
}

mod metered;
pub(in crate::cleanup_plan::replay) use metered::raw as raw_error_metered;
"#,
            r#"    }
    Ok(Diagnostic::io("SPX-H006", text))
}
"#,
        );
        undo(
            &mut current,
            r#"    lease: OracleLease,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    if !matches!(super::super::opted(), Ok(None)) {
        return Err(physical(
            lease,
            "opted oracle raw diagnostic requires its metered receiver",
        ));
    }
    let mut count = Count(0);
    if write!(count, "{message}").is_err() {
        return Err(physical(lease, "raw diagnostic length overflowed"));
"#,
            r#"    lease: OracleLease,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    let mut count = Count(0);
    if write!(count, "{message}").is_err() {
        return Err(physical(lease, "raw diagnostic length overflowed"));
"#,
        );
        undo(
            &mut current,
            r#"    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    match super::super::opted() {
        Ok(None) => {}
        Ok(Some(current)) if current == lease => return metered::replay(lease, function, message),
        Ok(Some(_)) | Err(_) => {
            return Err(physical(
                lease,
                "opted oracle diagnostic lease is unavailable or differs",
            ));
        }
    }
    let mut count = Count(0);
    if write!(
        count,
"#,
            r#"    function: &ResolvedFunction,
    message: fmt::Arguments<'_>,
) -> Result<Diagnostic, ReplayFailure> {
    let mut count = Count(0);
    if write!(
        count,
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__allocation__oracle__diagnostic.rs.before"
            ),
            "src/cleanup_plan/replay/allocation/oracle/diagnostic.rs"
        );
    }
    {
        let mut current = include_str!("../../factored.rs").to_owned();
        undo(
            &mut current,
            r#"        assert_eq!(short.allocated, short.freed);
    }
}

#[cfg(test)]
mod oracle_order;
"#,
            r#"        assert_eq!(short.allocated, short.freed);
    }
}
"#,
        );
        undo(
            &mut current,
            r#"        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
"#,
            r#"        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
"#,
        );
        undo(
            &mut current,
            r#"        &mut ReplayBudget::new(),
    )? {
        #[cfg(test)]
        oracle_hook::cross_check(program, function);
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
"#,
            r#"        &mut ReplayBudget::new(),
    )? {
        #[cfg(test)]
        cross_check(program, function);
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored.rs.before"
            ),
            "src/cleanup_plan/replay/factored.rs"
        );
    }
    {
        let mut current =
            include_str!("../../factored/borrowed_canonical/dictionary/keys.rs").to_owned();
        undo(
            &mut current,
            r#"        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        #[cfg(test)]
        if budget.oracle_lease().is_some() {
            return crate::cleanup_plan::replay::factored::oracle_order::label(
                self, other, function, budget,
            );
        }
        budget.charge(function, 1, "borrowed label actual kind")?;
        let (a, b) = match (self, other) {
            (Label::End(a), Label::End(b)) => return Ok(a.cmp(b)),
"#,
            r#"        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        budget.charge(function, 1, "borrowed label actual kind")?;
        let (a, b) = match (self, other) {
            (Label::End(a), Label::End(b)) => return Ok(a.cmp(b)),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored__borrowed_canonical__dictionary__keys.rs.before"
            ),
            "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"
        );
    }
    {
        let mut current = include_str!("../../factored/oracle_hook.rs").to_owned();
        undo(&mut current, r###"
// Classification of an undecided result carries no comparison authority.
fn require_decided(
    function: &ResolvedFunction,
    factored: &Result<Option<bool>, Diagnostic>,
) {
    assert!(
        !matches!(factored, Ok(None)),
        "funded factored skeleton comparison is undecided for `{}`",
        function.id,
    );
}

#[cfg(test)]
mod closure_controls;
"###, "");
        undo(&mut current, "        require_decided(function, &factored);\n", "");
        undo(
            &mut current,
            r#"            let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
            paid_sort(lease, &mut enumerating, function, &mut hir)?;
            paid_sort(lease, &mut enumerating, function, &mut plan)?;
            same_paths(function, &mut enumerating, &hir, &plan)
        })
    });
    // A fresh factored receiver runs regardless of the enumeration verdict.
"#,
            r#"            let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
            paid_sort(lease, &mut enumerating, function, &mut hir)?;
            paid_sort(lease, &mut enumerating, function, &mut plan)?;
            Ok(hir == plan)
        })
    });
    // A fresh factored receiver runs regardless of the enumeration verdict.
"#,
        );
        undo(
            &mut current,
            r#"/// Stable sort scratch is at most one element slot per path; pay that upper
/// bound on the account before the sort can allocate it.
fn paid_sort(
    _lease: OracleLease,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    paths: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    super::oracle_order::sort(paths, function, budget)
}
fn same_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    a: &[SkeletonPath],
    b: &[SkeletonPath],
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "oracle path vector length")?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (x, y) in a.iter().zip(b) {
        budget.charge(function, 1, "oracle path vector member")?;
        if super::oracle_order::path(x, y, function, budget)? != std::cmp::Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn cross_check_oracle(
"#,
            r#"/// Stable sort scratch is at most one element slot per path; pay that upper
/// bound on the account before the sort can allocate it.
fn paid_sort(
    lease: OracleLease,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    paths: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    let bytes = paths.len().checked_mul(std::mem::size_of::<SkeletonPath>());
    if bytes.is_none() {
        lease.fail_physical("skeleton sort scratch size overflowed");
    }
    match bytes.map(|bytes| lease.charge(bytes)) {
        Some(Ok(())) => {
            paths.sort();
            Ok(())
        }
        _ => {
            Err(budget.budget_refusal(function, "oracle account refused skeleton sort scratch", ""))
        }
    }
}

pub(super) fn cross_check_oracle(
"#,
        );
        undo(
            &mut current,
            r#"//! Explicitly financed test comparison hook. Ordinary unopted callers retain
//! the complete original enumerating/factored receiver. Owning tests opt into
//! one independent finite account without consuming their live builder ledger.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleLease};

/// Explicit private-test dispatcher. No compiler or resolver call chooses it.
pub(super) fn cross_check(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) {
    match super::super::allocation::opted() {
        Ok(None) => super::cross_check(program, function),
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
"#,
            r#"//! Pending test-only funded comparison receiver. Live `factored::cross_check`
//! retains its complete original body: neither actual owner control opts in.
//! Explicit private tests exercise this receiver while the documented transitive
//! acquisition audit remains open. It is not a fully funded live-hook claim.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleLease};

/// Explicit private-test dispatcher. No compiler or resolver call chooses it.
fn cross_check(program: &dyn crate::cleanup::ReplayContext, function: &ResolvedFunction) {
    match super::super::allocation::opted() {
        Ok(None) => super::cross_check(program, function),
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__factored__oracle_hook.rs.before"
            ),
            "src/cleanup_plan/replay/factored/oracle_hook.rs"
        );
    }
    {
        let mut current = include_str!("../../nested_shape/paid.rs").to_owned();
        undo(
            &mut current,
            r#"    }
    Ok(shape)
}

#[cfg(test)]
mod oracle;
"#,
            r#"    }
    Ok(shape)
}
"#,
        );
        undo(
            &mut current,
            r#"    physical: &mut ReplayBudget,
    scratch: &mut Scratch,
) -> Result<FieldLivenessShape, Diagnostic> {
    #[cfg(test)]
    if physical.oracle_lease().is_some() {
        return oracle::derive_reusing(
            program, function, root, next_flag, bounds, physical, scratch,
        );
    }
    let (depth, budget) = bounds;
    physical.reserve_clone(function, root, "independent shape root type")?;
    let mut lease = scratch.lease();
"#,
            r#"    physical: &mut ReplayBudget,
    scratch: &mut Scratch,
) -> Result<FieldLivenessShape, Diagnostic> {
    let (depth, budget) = bounds;
    physical.reserve_clone(function, root, "independent shape root type")?;
    let mut lease = scratch.lease();
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__nested_shape__paid.rs.before"
            ),
            "src/cleanup_plan/replay/nested_shape/paid.rs"
        );
    }
    {
        let mut current = include_str!("../../ordinary_inventory/substitute.rs").to_owned();
        undo(
            &mut current,
            r#"                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(
                            function,
                            budget,
                            format_args!(
                                "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                            ),
                        ));
                    }
                    if let Some(argument) = usize::try_from(*index)
                        .ok()
"#,
            r#"                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(function, budget, format_args!(
                            "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                        )));
                    }
                    if let Some(argument) = usize::try_from(*index)
                        .ok()
"#,
        );
        undo(
            &mut current,
            r#"                "oracle invocation authority refused a raw diagnostic",
            );
        }
        let result = if matches!(allocation::opted(), Ok(Some(current)) if current == lease) {
            allocation::raw_error_metered(lease, function, message)
        } else {
            allocation::raw_error_funded(lease, message)
        };
        return match result {
            Ok(error) => error,
            Err(refusal) => refusal.into_diagnostic(),
        };
"#,
            r#"                "oracle invocation authority refused a raw diagnostic",
            );
        }
        return match allocation::raw_error_funded(lease, message) {
            Ok(error) => error,
            Err(refusal) => refusal.into_diagnostic(),
        };
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__ordinary_inventory__substitute.rs.before"
            ),
            "src/cleanup_plan/replay/ordinary_inventory/substitute.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts.rs").to_owned();
        undo(
            &mut current,
            r#"mod inventory_lifecycle;

pub(super) mod inventory_summary;

#[cfg(test)]
pub(super) mod oracle_inventory;
"#,
            r#"mod inventory_lifecycle;

pub(super) mod inventory_summary;
"#,
        );
        undo(
            &mut current,
            r#"        }
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::needs_drop(program, function, ty, budget);
    }
    let mut payer = Payer {
        function,
"#,
            r#"        }
        return crate::cleanup::type_needs_resource_cleanup(program, ty)
            .map_err(|message| replay_error(function, message));
    }
    let mut payer = Payer {
        function,
"#,
        );
        undo(
            &mut current,
            r#"        }
        return Ok(index.type_flags(ty));
    }
    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        return oracle_inventory::flags(index, function, ty, budget);
    }
    index.type_flags_paid(
        ty,
        Payer {
"#,
            r#"        }
        return Ok(index.type_flags(ty));
    }
    index.type_flags_paid(
        ty,
        Payer {
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts/inventory_encoded.rs").to_owned();
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |_, payer: &mut Payer<'_>| {
                inventory_text::current(payer)?;
                Ok(true)
            },
            &mut work,
            false,
        );
    }
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

"#,
        );
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            true,
        );
    }
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
"#,
        );
        undo(
            &mut current,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            false,
        );
    }
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
"#,
            r#"    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
"#,
        );
        undo(
            &mut current,
            r#"pub(super) fn acquire(
    payer: &mut Payer<'_>,
) -> Result<ClassifierString, GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return ClassifierString::acquire_oracle_work(payer, &mut pay, &mut work);
    }
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
"#,
            r#"pub(super) fn acquire(
    payer: &mut Payer<'_>,
) -> Result<ClassifierString, GrowthFailure<Diagnostic>> {
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_encoded.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_encoded.rs"
        );
    }
    {
        let mut current =
            include_str!("../../paid_type_facts/inventory_encoded/tests.rs").to_owned();
        undo(
            &mut current,
            r#"}
#[test]
fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    let current = include_str!("../../../../../bounded_output/classifier_string.rs")
        .strip_suffix("\n#[cfg(test)]\nmod oracle_work;\n")
        .unwrap();
    let marker = "    /// A test-account caller authenticates its own invocation separately. This\n    /// detached marker contains no ACTIVE handle and grants no ordinary payer.\n    #[cfg(test)]\n    pub(crate) fn oracle_marker() -> Self {\n        Self(None)\n    }\n\n";
    assert_eq!(current.matches(marker).count(), 1);
    let current = current.replace(marker, "");
    assert_eq!(
        current.strip_suffix("\nmod inventory_work;\n").unwrap(),
        include_str!("../../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"#,
            r#"}
#[test]
fn full_classifier_string_original_body_remains_independent_of_work_receiver() {
    assert_eq!(
        include_str!("../../../../../bounded_output/classifier_string.rs")
            .strip_suffix("\nmod inventory_work;\n")
            .unwrap(),
        include_str!("../../../../../../docs/evidence/schema-summary/classifier_string.rs.before")
    );
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_encoded__tests.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_encoded/tests.rs"
        );
    }
    {
        let mut current = include_str!("../../paid_type_facts/inventory_tests.rs").to_owned();
        undo(
            &mut current,
            r#"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../../paid_type_facts.rs");
    let current = current
        .strip_suffix("\n#[cfg(test)]\npub(super) mod oracle_inventory;\n")
        .unwrap();
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::flags(index, function, ty, budget);\n    }\n",
        "",
    );
    let current = current.replace(
        "    #[cfg(test)]\n    if budget.oracle_lease().is_some() {\n        return oracle_inventory::needs_drop(program, function, ty, budget);\n    }\n",
        "",
    );
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
"#,
            r#"#[test]
fn full_current_classifier_source_equals_complete_saved_receiver_plus_raw_adapter() {
    let current = include_str!("../../paid_type_facts.rs");
    let (head, rest) = current
        .split_once("/// Raw ordinary inventory diagnostic adapter.")
        .unwrap();
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__paid_type_facts__inventory_tests.rs.before"
            ),
            "src/cleanup_plan/replay/paid_type_facts/inventory_tests.rs"
        );
    }
    {
        let mut current = include_str!("../../skeleton_work/oracle_entry.rs").to_owned();
        undo(
            &mut current,
            r#"                Err(error) => return Err(refused(budget, function, error)),
            }
        };
        let string_owner_moves =
            match crate::literal_format::checked_moving_string_operands_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(moves) => moves,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let string_condition_reads =
            match crate::string_ops::conditions::function_reads_funded_with_work(
                function, &mut units, &mut pay,
            ) {
                Ok(reads) => reads,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let requires = match crate::hir::lexical_exit::requires_funded_with_work(
            function, &mut pay, &mut units,
        ) {
            Ok(requires) => requires,
            Err(error) => return Err(refused(budget, function, error)),
        };
        // A semantic missing or ambiguous owner is the original absence; a
        // payer refusal never is.
        let owner = if requires {
            match crate::hir::lexical_exit::owner_funded_with_work(
                program, function, &mut pay, &mut units,
            ) {
                Ok(Ok(owner)) => Some(owner),
                Ok(Err(_)) => None,
                Err(error) => return Err(refused(budget, function, error)),
"#,
            r#"                Err(error) => return Err(refused(budget, function, error)),
            }
        };
        let string_owner_moves = match crate::literal_format::checked_moving_string_operands_funded(
            function, &mut pay,
        ) {
            Ok(moves) => moves,
            Err(error) => return Err(refused(budget, function, error)),
        };
        let string_condition_reads =
            match crate::string_ops::conditions::function_reads_funded(function, &mut pay) {
                Ok(reads) => reads,
                Err(error) => return Err(refused(budget, function, error)),
            };
        let requires = match crate::hir::lexical_exit::requires_funded(function, &mut pay) {
            Ok(requires) => requires,
            Err(error) => return Err(refused(budget, function, error)),
        };
        // A semantic missing or ambiguous owner is the original absence; a
        // payer refusal never is.
        let owner = if requires {
            match crate::hir::lexical_exit::owner_funded(program, function, &mut pay) {
                Ok(Ok(owner)) => Some(owner),
                Ok(Err(_)) => None,
                Err(error) => return Err(refused(budget, function, error)),
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__cleanup_plan__replay__skeleton_work__oracle_entry.rs.before"
            ),
            "src/cleanup_plan/replay/skeleton_work/oracle_entry.rs"
        );
    }
    {
        let mut current = include_str!("../../../../hir/function_value.rs").to_owned();
        undo(
            &mut current,
            r#"    };
    Ok((&INVOKE_ID, args, invocation_params(expression)?))
}

#[cfg(test)]
pub(crate) use funded::work::{walk_funded_paying_with_work, walk_funded_with_work};
"#,
            r#"    };
    Ok((&INVOKE_ID, args, invocation_params(expression)?))
}
"#,
        );
        assert_eq!(
            current,
            include_str!(
                "../../../../../docs/evidence/opt810-live-oracle/src__hir__function_value.rs.before"
            ),
            "src/hir/function_value.rs"
        );
    }
}
