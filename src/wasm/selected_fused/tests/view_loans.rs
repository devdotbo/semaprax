//! Leased view to loan binding for the selected Core-Wasm lowering. Authored,
//! unexecuted until the parent's whole batch runs.
//!
//! The canonical loan plan attaches two loans to the payload field read: the
//! view's own `SliceView` loan and the transient shared loan of the row Vec
//! operand, which ends right after the read. Only the former may end the view,
//! and every end it has (the operand after its last read and the short-circuit
//! join skipping that read) must be lowered.
use super::super::{accept, precursor::inspect, Refusal};
use super::{capacity, inputs, ORIGINAL};
use crate::hir::{ExpressionId, ResolvedProgram};
use crate::loan_plan::{Loan, LoanCause};

const OPERAND: LoanCause = LoanCause::BorrowedCall { argument: 0 };

/// Sites carrying a `SliceView` loan beside a Vec-operand shared loan.
fn payload_sites(program: &ResolvedProgram) -> Vec<ExpressionId> {
    let loans = &program.functions[0].loan_plan.loans;
    loans
        .iter()
        .filter(|loan| {
            loan.cause == LoanCause::SliceView
                && loans
                    .iter()
                    .any(|other| other.site == loan.site && other.cause == OPERAND)
        })
        .map(|loan| loan.site.clone())
        .collect()
}

fn at<'p>(program: &'p ResolvedProgram, site: &ExpressionId, cause: &LoanCause) -> Vec<&'p Loan> {
    program.functions[0]
        .loan_plan
        .loans
        .iter()
        .filter(|loan| &loan.site == site && &loan.cause == cause)
        .collect()
}

#[test]
fn leased_payload_view_binds_only_its_slice_loan_and_lowers_every_end() {
    let source = crate::parse(ORIGINAL, "selected-fused-view-loans.spx").unwrap();
    let owner = crate::hir::selected_fused_candidate::validate_selected(&source)
        .expect("original source passes full selected validation");
    let program = owner.program();
    let sites = payload_sites(program);
    assert_eq!(sites.len(), 1, "exactly one leased payload read");
    let [view] = at(program, &sites[0], &LoanCause::SliceView)[..] else {
        panic!("one view loan at the payload read")
    };
    let [operand] = at(program, &sites[0], &OPERAND)[..] else {
        panic!("one Vec-operand loan at the payload read")
    };
    // The operand loan ends before any end of the view loan; matching by site
    // alone would end the view before its first `view_len`.
    assert!(operand.ends.iter().all(|end| !view.ends.contains(end)));
    // The view's last reads sit behind `&&` and inside a match, so its loan
    // ends on more than one path. Every end destination is dead on every
    // entering edge, so a guarded end there is exact on each executed path.
    assert!(view.ends.len() >= 2, "{:?}", view.ends);
    let plan = &program.functions[0].loan_plan;
    for edge in &view.end_edges {
        let to = plan.edges[usize::from(*edge)].to;
        assert!(!plan.endpoints[usize::from(to)]
            .live_before
            .contains(&view.id));
        assert!(plan
            .edges
            .iter()
            .filter(|entering| entering.to == to)
            .all(|entering| !entering.live.contains(&view.id)));
    }
    let first = accept(&owner).expect("sealed owner lowers with every view end");
    assert_eq!(first.bytes, accept(&owner).unwrap().bytes);
}

/// The view's last read is the operand of `byte_get`, and its loan ends at that
/// operand's after point. Every consuming `view_len`/`view_byte` must receive
/// a token staged straight from its local with no `end_borrow` in between;
/// ending the view at the operand itself retired the token the call then read
/// (the host's `stale` refusal on the original 4096 rows).
#[test]
fn consuming_view_calls_receive_unretired_tokens() {
    use super::super::import;
    use wasmparser::Operator as O;
    let source = crate::parse(ORIGINAL, "selected-fused-view-consumers.spx").unwrap();
    let owner = crate::hir::selected_fused_candidate::validate_selected(&source)
        .expect("original source passes full selected validation");
    let artifact = accept(&owner).expect("sealed owner lowers");
    let mut ops = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&artifact.bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut reader = body.get_operators_reader().unwrap();
            while !reader.eof() {
                ops.push(reader.read().unwrap());
            }
        }
    }
    let ends = |range: &[O<'_>]| {
        range.iter().any(
            |op| matches!(op, O::Call { function_index } if *function_index == import::END_BORROW),
        )
    };
    // Token operand distance from the call: (domain, token) or (domain, token, index).
    let mut consumers = [0usize; 2];
    for (at, op) in ops.iter().enumerate() {
        let O::Call { function_index } = op else {
            continue;
        };
        let operand = match *function_index {
            import::VIEW_LEN => 1,
            import::VIEW_BYTE => 2,
            _ => continue,
        };
        let O::LocalGet {
            local_index: staged,
        } = &ops[at - operand]
        else {
            panic!("consuming call reads a staged token local")
        };
        let set = ops[..at]
            .iter()
            .rposition(|op| matches!(op, O::LocalSet { local_index } if local_index == staged))
            .expect("token staged before its consuming call");
        assert!(
            matches!(ops[set - 1], O::LocalGet { .. }),
            "token staged straight from its view local, not after an end: {:?}",
            ops[set - 1]
        );
        assert!(!ends(&ops[set..at]), "view ended before its consuming call");
        assert!(
            ends(&ops[at + 1..]),
            "consumed view still ends after its call"
        );
        consumers[operand - 1] += 1;
    }
    assert!(consumers.iter().all(|count| *count >= 1), "{consumers:?}");
}

#[test]
fn forged_view_loans_at_the_payload_read_refuse_before_lowering() {
    let selected = inputs(4096);
    let summary = capacity(&selected);
    let sites = payload_sites(&selected.program);
    assert_eq!(sites.len(), 1);
    let index = |program: &ResolvedProgram| {
        program.functions[0]
            .loan_plan
            .loans
            .iter()
            .position(|loan| loan.site == sites[0] && loan.cause == LoanCause::SliceView)
            .unwrap()
    };
    // The view loan relabelled as a second operand loan: no view loan remains.
    let mut relabelled = selected.program.clone();
    let view = index(&relabelled);
    relabelled.functions[0].loan_plan.loans[view].cause = OPERAND;
    // A duplicated view loan is ambiguous.
    let mut duplicated = selected.program.clone();
    let view = index(&duplicated);
    let copy = duplicated.functions[0].loan_plan.loans[view].clone();
    duplicated.functions[0].loan_plan.loans.push(copy);
    // Dropping all but the first end leaves the other paths unended.
    let mut truncated = selected.program.clone();
    let view = index(&truncated);
    assert!(truncated.functions[0].loan_plan.loans[view].ends.len() >= 2);
    truncated.functions[0].loan_plan.loans[view]
        .ends
        .truncate(1);
    truncated.functions[0].loan_plan.loans[view]
        .end_edges
        .truncate(1);
    for forged in [relabelled, duplicated, truncated] {
        assert_eq!(
            inspect(&selected.source, &forged, &summary).err(),
            Some(Refusal::Loans)
        );
    }
    assert!(inspect(&selected.source, &selected.program, &summary).is_ok());
}

#[test]
fn lowering_ends_views_by_loan_identity_not_by_site() {
    let lower = include_str!("../lower.rs");
    for guard in [
        "loan.cause == LoanCause::SliceView",
        "if loans.next().is_some() || loan.end_edges.is_empty() {",
        "for edge in &loan.end_edges {",
        "if edge.live.contains(&loan.id)",
        "|| to.live_before.contains(&loan.id)",
        "|| !to.kills.contains(&loan.id)",
        ".any(|entering| entering.to == edge.to && entering.live.contains(&loan.id))",
        "if ends != loan.ends {",
        "usize::from(edge.to) == point && !edge.live.contains(id)",
        "if !std::ptr::eq(view.loan, loan) {",
        "&& !view.lowered[index]",
        ".any(|view| view.lowered.contains(&false))",
        ".any(|((_, _, ended), covered)| !covered && !ended)",
    ] {
        assert!(lower.contains(guard), "{guard}");
    }
    assert!(!lower.contains("*site == &loan.site"));
    assert!(!lower.contains("scope.views.retain("));
    let suffix = include_str!("../suffix.rs");
    assert!(suffix.contains("self.open_view(view, &e.id)?;"));
    // A view operand's own after-point end waits for its consuming call, and
    // a failure exit in between still retires it.
    for guard in [
        "let carrier = self.consumed_operand(view)?;",
        "let ending = self.loan_ends(&e.id, LoanPointPhase::After);",
        "self.release_consumed(mark);",
    ] {
        assert!(suffix.contains(guard), "{guard}");
    }
    assert_eq!(suffix.matches("self.release_consumed(mark);").count(), 2);
    for guard in [
        "for view in &self.consumed {",
        "for view in self.consumed.split_off(mark) {",
        ".any(|view| self.consumed.contains(&view.local))",
    ] {
        assert!(lower.contains(guard), "{guard}");
    }
}
