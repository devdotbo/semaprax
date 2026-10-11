//! Leased view to loan binding for the selected Core-Wasm lowering. Authored,
//! unexecuted until the parent's whole batch runs.
//!
//! The canonical loan plan attaches two loans to the payload field read: the
//! view's own `SliceView` loan and the transient shared loan of the row Vec
//! operand, which ends right after the read. Only the former may end the view,
//! and every end it has (the operand after its last read and the short-circuit
//! join skipping that read) must be lowered.
use super::super::{accept, precursor::inspect, Refusal};
use super::{capacity, inputs, nested, CHECKED_INDEX, NESTED_LEN, ORIGINAL};
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

fn sealed_bytes(text: &str) -> Vec<u8> {
    let source = crate::parse(text, "selected-fused-view-consumers.spx").unwrap();
    let owner = crate::hir::selected_fused_candidate::validate_selected(&source)
        .expect("sealed source passes full selected validation");
    accept(&owner).expect("sealed owner lowers").bytes
}

fn operators(bytes: &[u8]) -> Vec<wasmparser::Operator<'_>> {
    let mut ops = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut reader = body.get_operators_reader().unwrap();
            while !reader.eof() {
                ops.push(reader.read().unwrap());
            }
        }
    }
    ops
}

fn calls(op: &wasmparser::Operator<'_>, function: u32) -> bool {
    matches!(op, wasmparser::Operator::Call { function_index } if *function_index == function)
}

/// Failure exits as `[select_failure, return_failure]` operator ranges; their
/// code never falls through, so normal-path ordering ignores them.
fn failure_exits(ops: &[wasmparser::Operator<'_>]) -> Vec<std::ops::Range<usize>> {
    use super::super::import;
    let mut exits = Vec::new();
    for (at, op) in ops.iter().enumerate() {
        if calls(op, import::SELECT_FAILURE) {
            let end = at
                + ops[at..]
                    .iter()
                    .position(|op| calls(op, import::RETURN_FAILURE))
                    .expect("failure exit returns its selected failure");
            assert!(
                matches!(ops[end + 1], wasmparser::Operator::Unreachable),
                "no result publication after a failure exit"
            );
            exits.push(at..end + 1);
        }
    }
    exits
}

/// Every consuming `view_len`/`view_byte` receives a token staged straight
/// from its local with no normal-path `end_borrow` before the call. Returns
/// each consumer's (token staging, call) positions by kind.
fn consumer_control(ops: &[wasmparser::Operator<'_>]) -> [Vec<(usize, usize)>; 2] {
    use super::super::import;
    use wasmparser::Operator as O;
    let exits = failure_exits(ops);
    let normal = |at: usize| !exits.iter().any(|exit| exit.contains(&at));
    let ends = |range: std::ops::Range<usize>| {
        range
            .into_iter()
            .any(|at| normal(at) && calls(&ops[at], import::END_BORROW))
    };
    // Token operand distance from the call: (domain, token) or (domain, token, index).
    let mut consumers = [Vec::new(), Vec::new()];
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
        assert!(!ends(set..at), "view ended before its consuming call");
        assert!(
            ends(at + 1..ops.len()),
            "consumed view still ends after its call"
        );
        consumers[operand - 1].push((set, at));
    }
    assert!(
        consumers.iter().all(|kind| !kind.is_empty()),
        "{consumers:?}"
    );
    consumers
}

/// The view's last read is the operand of `byte_get`, and its loan ends at that
/// operand's after point. Ending the view at the operand itself retired the
/// token the call then read (the host's `stale` refusal on the original 4096
/// rows). The sealed same-token variants move that last read into a nested
/// `byte_len` of the same view inside the outer `byte_get`'s index: the inner
/// call's own release must not retire the token the outer call already holds.
#[test]
fn consuming_view_calls_receive_unretired_tokens() {
    let original = consumer_control(&operators(&sealed_bytes(ORIGINAL)));
    for replacement in [NESTED_LEN, CHECKED_INDEX] {
        let module = sealed_bytes(&nested(replacement));
        let [lens, bytes] = consumer_control(&operators(&module));
        assert_eq!(
            lens.len(),
            original[0].len() + 1,
            "one more nested view_len"
        );
        assert_eq!(bytes.len(), original[1].len());
        // A same-token nesting: some `view_len` runs while an outer
        // `view_byte` holds its staged token, and the control above already
        // proved no normal-path end separates that outer staging from its call.
        assert!(
            bytes
                .iter()
                .any(|(set, call)| lens.iter().any(|(_, inner)| set < inner && inner < call)),
            "nested view_len inside an outer view_byte's hold"
        );
    }
}

/// `byte_get(data, byte_len(data) - 1usize)`: the view's loan ends inside the
/// index, before the checked subtraction, while the outer `view_byte` holds the
/// token. The subtraction's exact failure exit (sticky first selection, no
/// result publication) retires that deferred view exactly once; the normal
/// path retires it once, right after the outer call.
#[test]
fn checked_index_failure_retires_each_held_token_once() {
    use super::super::import;
    use crate::wasm::aggregate::STATUS_SUB_OVERFLOW;
    use wasmparser::Operator as O;
    let module = sealed_bytes(&nested(CHECKED_INDEX));
    let ops = operators(&module);
    let [lens, bytes] = consumer_control(&ops);
    let exits = failure_exits(&ops);
    let (set, call) = *bytes
        .iter()
        .find(|(set, call)| lens.iter().any(|(_, inner)| set < inner && inner < call))
        .expect("outer view_byte holding the nested view_len's view");
    // The view the outer call's token came from is the one retired first
    // after that call, on the normal path.
    let retired = |range: std::ops::Range<usize>| -> Vec<u32> {
        range
            .filter(|at| calls(&ops[*at], import::END_BORROW))
            .map(|at| match ops[at - 1] {
                O::LocalGet { local_index } => local_index,
                ref other => panic!("end_borrow of a view local, not {other:?}"),
            })
            .collect()
    };
    let first = (call + 1..ops.len())
        .find(|at| calls(&ops[*at], import::END_BORROW))
        .expect("held view retires after its outer call");
    let [view] = retired(first..first + 1)[..] else {
        panic!("one view retired")
    };
    assert!(
        !exits.iter().any(|exit| exit.contains(&first)),
        "normal-path retirement"
    );
    let subtraction: Vec<std::ops::Range<usize>> = exits
        .iter()
        .filter(|exit| set < exit.start && exit.end <= call)
        .filter(|exit| {
            matches!(ops[exit.start - 1], O::I32Const { value } if value == STATUS_SUB_OVERFLOW)
        })
        .cloned()
        .collect();
    let [exit] = &subtraction[..] else {
        panic!("exactly one checked-subtraction exit inside the outer hold")
    };
    let ended = retired(exit.clone());
    assert_eq!(
        ended.iter().filter(|local| **local == view).count(),
        1,
        "held token retired exactly once on the failure route: {ended:?}"
    );
    let unique = ended.iter().collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        unique.len(),
        ended.len(),
        "no view retired twice: {ended:?}"
    );
    // Sticky selection precedes every end; the exit never publishes a result.
    assert!(calls(&ops[exit.start], import::SELECT_FAILURE));
    assert!(!ops[exit.clone()]
        .iter()
        .any(|op| calls(op, import::VIEW_BYTE)));
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
    // A consuming call holds its operand's exact view from the read on; any
    // end of that view reached meanwhile, nested or not, defers until no hold
    // remains, and a failure exit in between still retires it once.
    for guard in [
        "let carrier = self.consumed_operand(view)?;",
        "let view = self.view_of(e)?;",
        "self.holds.push(view);",
        "let mark = self.holds.len();",
        "self.release(mark);",
    ] {
        assert!(suffix.contains(guard), "{guard}");
    }
    assert_eq!(suffix.matches("self.release(mark);").count(), 2);
    assert_eq!(suffix.matches("self.holds.push(view);").count(), 1);
    // The hold precedes the operand's own after-point ends.
    let operand = suffix.split_once("fn consumed_operand(").unwrap().1;
    assert!(
        operand.find("self.holds.push(view);").unwrap()
            < operand
                .find("self.loan_points(&e.id, LoanPointPhase::After);")
                .unwrap()
    );
    assert!(!suffix.contains("loan_ends"));
    for guard in [
        "if !self.holds.contains(&view) {",
        "} else if !self.deferred.contains(&view) {",
        "for view in &self.deferred {",
        "self.holds.truncate(mark);",
        ".partition(|view| !self.holds.contains(view));",
        "self.holds.contains(&view.local) || self.deferred.contains(&view.local)",
        "if !self.holds.is_empty() || !self.deferred.is_empty() {",
        ".filter(|view| view.loan.site == e.id)",
        ".and_then(|binding| binding.view),",
        "if !open().any(|candidate| candidate.local == view) {",
    ] {
        assert!(lower.contains(guard), "{guard}");
    }
    assert!(!lower.contains("self.consumed"));
}
