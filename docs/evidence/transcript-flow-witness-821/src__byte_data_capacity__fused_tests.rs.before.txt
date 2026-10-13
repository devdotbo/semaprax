//! Owning gates for the explicit `FusedConstructionV1` capacity rule. These
//! flows are hand-built: no source/HIR selector constructs them yet, so they
//! prove only the accounting gate, never an admitted program.
use super::fused::MAX_FUSED_ROWS;
use super::*;

const FUSED: AllocationStrategy = AllocationStrategy::FusedConstructionV1;
const MAX_VALUE: u64 = crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES;
const PRODUCER: &str = "row.payload";
const MISMATCH: &str = "fused construction producer is not the exact unconditional body producer";
const BODY: &str = "fused construction body allocates outside its single producer";
const CONDITION: &str = "fused construction condition reaches an allocation";

fn function(identity: &str, execution: CapacityFlow) -> FunctionCapacityInput {
    FunctionCapacityInput {
        function: identity.to_owned(),
        array_slots: Vec::new(),
        execution,
    }
}

fn copy(site: &str, bytes: u64) -> CapacityFlow {
    CapacityFlow::BytesCopy {
        site: site.to_owned(),
        conservative_payload_bytes: bytes,
    }
}

fn call(site: &str, callee: &str) -> CapacityFlow {
    CapacityFlow::Call {
        site: site.to_owned(),
        callee: callee.to_owned(),
    }
}

fn fused(rows: u32, width: u64, condition: CapacityFlow, body: CapacityFlow) -> CapacityFlow {
    CapacityFlow::FusedConstruction {
        producer: PRODUCER.to_owned(),
        rows,
        width,
        condition: Box::new(condition),
        body: Box::new(body),
    }
}

/// The body producer of the selected shape.
fn row(width: u64) -> CapacityFlow {
    CapacityFlow::Sequence(vec![copy(PRODUCER, width)])
}

/// The exact selected shape: one unconditional body producer of `width`.
fn exact(rows: u32, width: u64) -> CapacityFlow {
    fused(rows, width, CapacityFlow::Empty, row(width))
}

/// A body of exactly `nodes` flow nodes: one Sequence, its producer and Empty leaves.
fn wide_body(nodes: usize) -> CapacityFlow {
    let mut children = vec![copy(PRODUCER, 3)];
    children.resize(nodes - 1, CapacityFlow::Empty);
    CapacityFlow::Sequence(children)
}

fn ordinary(count: usize, bytes: u64) -> Vec<CapacityFlow> {
    (0..count)
        .map(|index| copy(&format!("ordinary.{index}"), bytes))
        .collect()
}

/// Summary of a single selected `main` function.
fn charged(execution: CapacityFlow) -> FunctionCapacitySummary {
    summary(&[function("main", execution)], "main")
}

fn summary(functions: &[FunctionCapacityInput], identity: &str) -> FunctionCapacitySummary {
    let program = analyze_with_strategy(functions, FUSED).expect("fits");
    assert_eq!(program.strategy(), FUSED);
    program.function(identity).expect("summary").clone()
}

fn rejects(
    functions: &[FunctionCapacityInput],
    strategy: AllocationStrategy,
    diagnostic: CapacityDiagnostic,
    needle: &str,
) {
    let result = analyze_with_strategy(functions, strategy);
    let error = result.expect_err(needle);
    assert_eq!(error.diagnostic, diagnostic, "{error}");
    assert!(error.detail.contains(needle), "{error}");
}

fn rejects_main(execution: CapacityFlow, diagnostic: CapacityDiagnostic, needle: &str) {
    rejects(&[function("main", execution)], FUSED, diagnostic, needle);
}

#[test]
fn fused_construction_charges_exactly_one_backing_event_and_its_whole_extent() {
    let selected = charged(exact(4096, 3));
    assert_eq!(selected.bytes_copy_sites, 1);
    assert_eq!(selected.fused_backing_events, 1);
    assert_eq!(selected.owned_byte_payload_bytes, 12_288);

    // A caller inherits the charge through the ordinary call summary.
    let caller = summary(
        &[
            function("main", call("main.build", "build")),
            function("build", exact(8, 3)),
        ],
        "main",
    );
    assert_eq!(caller.bytes_copy_sites, 1);
    assert_eq!(caller.fused_backing_events, 1);
    assert_eq!(caller.owned_byte_payload_bytes, 24);
}

#[test]
fn selected_strategy_leaves_every_ordinary_summary_unchanged() {
    let program = [
        function(
            "main",
            CapacityFlow::Sequence(vec![copy("a", 7), call("main.helper", "helper")]),
        ),
        function(
            "helper",
            CapacityFlow::Alternative(vec![copy("b", 3), copy("c", 9)]),
        ),
    ];
    let legacy = analyze(&program).unwrap();
    let selected = analyze_with_strategy(&program, FUSED).unwrap();
    assert_eq!(legacy.strategy(), AllocationStrategy::Legacy);
    assert_eq!(selected.strategy(), FUSED);
    assert!(legacy.functions().eq(selected.functions()));
    assert!(legacy
        .functions()
        .all(|(_, summary)| summary.fused_backing_events == 0));
}

#[test]
fn fused_event_joins_the_unchanged_32_event_path_limit() {
    let mut fits = ordinary(31, 1);
    fits.push(exact(4096, 3));
    let fits = charged(CapacityFlow::Sequence(fits));
    assert_eq!(fits.bytes_copy_sites, MAX_BYTES_COPY_SITES);
    assert_eq!(fits.owned_byte_payload_bytes, 31 + 12_288);

    let mut over = ordinary(32, 1);
    over.push(exact(4096, 3));
    rejects_main(
        CapacityFlow::Sequence(over),
        CapacityDiagnostic::Allocation,
        "bytes_copy path reaches 33 sites; limit is 32",
    );
}

#[test]
fn fused_extent_joins_the_unchanged_cumulative_payload_limit() {
    // Fifteen maximal ordinary values plus one maximal backing is exactly 2 MiB.
    let mut limit = ordinary(15, MAX_VALUE);
    limit.push(exact(MAX_FUSED_ROWS, 32));
    let fits = charged(CapacityFlow::Sequence(limit.clone()));
    assert_eq!(fits.owned_byte_payload_bytes, MAX_OWNED_BYTE_PAYLOAD_BYTES);

    limit.push(copy("one.more", 1));
    rejects_main(
        CapacityFlow::Sequence(limit),
        CapacityDiagnostic::Allocation,
        "bytes_copy path admits 2097153 payload bytes; limit is 2097152",
    );
}

#[test]
fn row_extent_and_arithmetic_bounds_are_exact() {
    assert_eq!(MAX_FUSED_ROWS, 4_096);
    for (rows, width) in [(MAX_FUSED_ROWS, 1), (MAX_FUSED_ROWS, 32), (1, MAX_VALUE)] {
        let extent = charged(exact(rows, width)).owned_byte_payload_bytes;
        assert_eq!(extent, u64::from(rows) * width);
    }
    let allocation = CapacityDiagnostic::Allocation;
    rejects_main(
        exact(MAX_FUSED_ROWS + 1, 1),
        allocation,
        "fused construction reaches 4097 rows; limit is 4096",
    );
    rejects_main(
        exact(1, MAX_VALUE + 1),
        allocation,
        "backing extent 131073 exceeds the per-value limit 131072",
    );
    rejects_main(
        exact(MAX_FUSED_ROWS, 33),
        allocation,
        "backing extent 135168 exceeds the per-value limit 131072",
    );
    rejects_main(
        exact(2, u64::MAX),
        allocation,
        "fused construction backing extent overflowed",
    );
}

#[test]
fn producers_outside_the_single_body_producer_are_refused() {
    let allocation = CapacityDiagnostic::Allocation;
    let helper = function("helper", copy("helper.copy", 1));
    let stdin = CapacityFlow::StdinRead {
        site: "body.stdin".to_owned(),
        conservative_payload_bytes: 1,
    };
    for extra in [copy("extra", 3), call("body.call", "helper"), stdin] {
        let body = CapacityFlow::Sequence(vec![copy(PRODUCER, 3), extra]);
        let main = function("main", fused(8, 3, CapacityFlow::Empty, body));
        rejects(&[main, helper.clone()], FUSED, allocation, BODY);
    }
    for condition in [copy("condition", 1), call("condition.call", "helper")] {
        let main = function("main", fused(8, 3, condition, row(3)));
        rejects(&[main, helper.clone()], FUSED, allocation, CONDITION);
    }
}

#[test]
fn body_scan_continues_the_existing_flow_node_bound_exactly() {
    let bound = "capacity flow exceeds the compiler node bound";
    // The scan resumes the ordinary walk's count: prior nodes plus every body
    // node may equal the bound, and one more refuses before the stack grows.
    let body = wide_body(1_000);
    let scan = |nodes| super::fused::validate("main", FUSED, PRODUCER, 8, 3, &body, nodes);
    assert!(scan(MAX_FLOW_NODES - 1_000).is_ok());
    for nodes in [MAX_FLOW_NODES - 999, MAX_FLOW_NODES, usize::MAX] {
        let error = scan(nodes).expect_err(bound);
        assert_eq!(error.diagnostic, CapacityDiagnostic::Invariant);
        assert_eq!(error.detail, bound);
    }

    // Whole entry: the fused node, its condition and body total exactly the bound.
    let at_bound = fused(8, 3, CapacityFlow::Empty, wide_body(MAX_FLOW_NODES - 2));
    assert_eq!(charged(at_bound).fused_backing_events, 1);
    let over = fused(8, 3, CapacityFlow::Empty, wide_body(MAX_FLOW_NODES - 1));
    rejects_main(over, CapacityDiagnostic::Invariant, bound);
}

#[test]
fn unselected_fused_flow_is_an_invariant_on_default_and_legacy_entries() {
    // Proof data alone grants nothing.
    let forged = [function("main", exact(8, 3))];
    let needle = "requires the explicit FusedConstructionV1 allocation strategy";
    let error = analyze(&forged).expect_err(needle);
    assert_eq!(error.diagnostic.code(), "SPX-H006");
    assert!(error.detail.contains(needle), "{error}");
    let legacy = AllocationStrategy::Legacy;
    rejects(&forged, legacy, CapacityDiagnostic::Invariant, needle);
}

#[test]
fn forged_producer_shape_and_nesting_are_invariants() {
    let invariant = CapacityDiagnostic::Invariant;
    let empty = || CapacityFlow::Empty;
    let conditional = CapacityFlow::Alternative(vec![row(3), empty()]);
    let duplicate = CapacityFlow::Sequence(vec![row(3), row(3)]);
    rejects_main(fused(8, 3, empty(), row(4)), invariant, MISMATCH);
    rejects_main(fused(8, 3, empty(), conditional), invariant, MISMATCH);
    rejects_main(fused(8, 3, empty(), copy("other", 3)), invariant, MISMATCH);
    rejects_main(fused(8, 3, empty(), duplicate), invariant, MISMATCH);

    let positive = "fused construction needs positive rows and width";
    rejects_main(exact(0, 3), invariant, positive);
    rejects_main(exact(8, 0), invariant, positive);
    let nested = CapacityFlow::Sequence(vec![copy(PRODUCER, 3), exact(2, 3)]);
    let nested = fused(8, 3, empty(), nested);
    rejects_main(nested, invariant, "nested fused construction is closed");
    let repeated = CapacityFlow::FiniteOwnedLoop {
        iterations: 2,
        condition: Box::new(empty()),
        body: Box::new(exact(8, 3)),
    };
    let repeat = "fused construction cannot repeat inside a finite iterator";
    rejects_main(repeated, invariant, repeat);

    // An ordinary while still refuses any reachable allocation, fused or not.
    let looped = CapacityFlow::Loop {
        condition: Box::new(empty()),
        body: Box::new(exact(8, 3)),
    };
    let reachable = "bytes_copy is reachable from a while condition or body";
    rejects_main(looped, CapacityDiagnostic::Allocation, reachable);
}
