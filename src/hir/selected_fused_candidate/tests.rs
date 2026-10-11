use super::*;
use crate::byte_data_capacity::{
    analyze_with_strategy, AllocationStrategy, CapacityDiagnostic, CapacityFlow,
    FunctionCapacityInput,
};
use crate::hir::ResolvedStatement;

const SOURCE: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");
const PATH: &str = "selected-fused-candidate.spx";

fn parsed(source: &str) -> ast::Program {
    crate::parse(source, PATH).unwrap()
}

fn publish(source: &ast::Program) -> ValidatedFusedSelection<'_> {
    match validate_selected(source) {
        Ok(owner) => owner,
        Err(refusal) => panic!("selected factory refused: {refusal:?}"),
    }
}

fn main_statements(program: &ResolvedProgram) -> &[ResolvedStatement] {
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let crate::hir::ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("main body block")
    };
    statements
}

fn fused_node(flow: &mut CapacityFlow) -> Option<&mut CapacityFlow> {
    match flow {
        CapacityFlow::FusedConstruction { .. } => Some(flow),
        CapacityFlow::Sequence(children) | CapacityFlow::Alternative(children) => {
            children.iter_mut().find_map(fused_node)
        }
        CapacityFlow::Loop { condition, body }
        | CapacityFlow::FiniteOwnedLoop {
            condition, body, ..
        } => fused_node(condition).or_else(|| fused_node(body)),
        _ => None,
    }
}

fn main_input(inputs: &mut [FunctionCapacityInput]) -> &mut FunctionCapacityInput {
    inputs
        .iter_mut()
        .find(|input| input.function == "app.main")
        .unwrap()
}

#[test]
fn ordinary_32_row_control_keeps_the_unchanged_legacy_routes() {
    let source = SOURCE.replace("4096", "32");
    let program = parsed(&source);
    assert!(crate::check(&source, PATH).is_ok());
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let summary = crate::hir::analyze_byte_data_capacity(&resolved).unwrap();
    let main = summary.function("app.main").unwrap();
    assert_eq!(
        (main.bytes_copy_sites, main.fused_backing_events),
        (32, 0)
    );
    assert!(matches!(
        validate_selected(&program),
        Err(SelectedRefusal::Source(CandidateRefusal::NoSelection))
    ));
}

#[test]
fn original_4096_publishes_only_after_full_selected_hir_validation() {
    let program = parsed(SOURCE);
    // The ordinary public routes are unchanged.
    assert!(crate::check(SOURCE, PATH).is_err());
    assert!(crate::hir::resolve(&program).is_err());

    let owner = publish(&program);
    assert!(std::ptr::eq(owner.source(), &program));
    assert_eq!(
        owner.capacity(),
        SelectedCapacity {
            rows: 4096,
            width: 3,
            extent: 12_288,
            bytes_copy_sites: 1,
            fused_backing_events: 1,
            owned_byte_payload_bytes: 12_288,
        }
    );
    // The ordinary validator still refuses the same typed program.
    assert!(crate::hir::validate(owner.program()).is_err());

    // Same typed shape as the ordinary 32-row control, original count kept.
    let control_source = parsed(&SOURCE.replace("4096", "32"));
    let control = crate::hir::resolve(&control_source).unwrap();
    let (selected, ordinary) = (main_statements(owner.program()), main_statements(&control));
    assert_eq!(selected.len(), ordinary.len());
    assert!(matches!(selected[2], ResolvedStatement::While { .. }));
    assert!(matches!(ordinary[2], ResolvedStatement::While { .. }));

    let joined = owner
        .with_joined(|joined| {
            assert!(joined.still_current(owner.source(), owner.program()));
            let flow = joined.flow();
            assert_eq!(flow.producer, joined.selection().producer().id.as_str());
            (flow.rows, flow.width, joined.proof().rows())
        })
        .unwrap();
    assert_eq!(joined, (4096, 3, 4096));
    println!("validated private FusedConstructionV1 selection; no default activation");
}

#[test]
fn decoded_stale_and_foreign_hir_cannot_join_select_or_validate() {
    let program = parsed(SOURCE);
    let owner = publish(&program);

    // Decoded copy: equal bytes, different program.
    let bytes = crate::cache_codec::encode(owner.program()).unwrap();
    let decoded: ResolvedProgram = crate::cache_codec::decode(&bytes).unwrap();
    owner
        .with_joined(|joined| {
            let selected = Selected::from_joined(joined);
            assert!(selected.held_by(owner.program()));
            assert!(!selected.held_by(&decoded));
            assert!(crate::hir::byte_capacity::analyze_selected_byte_data_capacity(
                &decoded, selected
            )
            .is_err());
            assert!(
                crate::hir::validate_selected_with_static_text(&decoded, Some(selected)).is_err()
            );
        })
        .unwrap();

    // Stale producer from an equal-text parse is not the current source node.
    let stale_source = parsed(SOURCE);
    let stale = check_candidate(&stale_source).unwrap();
    assert!(matches!(
        resolution::join_current(&program, stale.proof().producer().call, owner.program()),
        Err(Refusal::Join(_))
    ));

    // Foreign program: a selection joined over another owner's HIR selects
    // nothing in this one.
    let foreign_source = parsed(SOURCE);
    let foreign = publish(&foreign_source);
    foreign
        .with_joined(|joined| {
            let selected = Selected::from_joined(joined);
            assert!(!selected.held_by(owner.program()));
            assert!(crate::hir::byte_capacity::analyze_selected_byte_data_capacity(
                owner.program(),
                selected
            )
            .is_err());
        })
        .unwrap();
}

#[test]
fn selected_capacity_events_are_exact_and_forged_producers_or_rescales_refuse() {
    let program = parsed(SOURCE);
    let owner = publish(&program);
    owner
        .with_joined(|joined| {
            let selected = Selected::from_joined(joined);
            let projected =
                crate::hir::byte_capacity::selected_inputs_for_test(owner.program(), selected)
                    .unwrap();
            let extra = |count: usize| {
                let mut inputs = projected.clone();
                let CapacityFlow::Sequence(execution) = &mut main_input(&mut inputs).execution
                else {
                    panic!("function execution sequence")
                };
                for index in 0..count {
                    execution.push(CapacityFlow::BytesCopy {
                        site: format!("test.extra.{index}"),
                        conservative_payload_bytes: 1,
                    });
                }
                inputs
            };
            // One fused event plus 31 ordinary events fit the unchanged cap.
            let fits = analyze_with_strategy(&extra(31), AllocationStrategy::FusedConstructionV1)
                .unwrap();
            let main = fits.function("app.main").unwrap();
            assert_eq!(
                (main.bytes_copy_sites, main.fused_backing_events),
                (32, 1)
            );
            let short = analyze_with_strategy(&extra(32), AllocationStrategy::FusedConstructionV1)
                .unwrap_err();
            assert_eq!(short.diagnostic, CapacityDiagnostic::Allocation);

            // A renamed producer is not the body's exact producer.
            let mut forged = projected.clone();
            let Some(CapacityFlow::FusedConstruction { producer, .. }) =
                fused_node(&mut main_input(&mut forged).execution)
            else {
                panic!("projected fused construction")
            };
            producer.push_str(".forged");
            let refused =
                analyze_with_strategy(&forged, AllocationStrategy::FusedConstructionV1)
                    .unwrap_err();
            assert_eq!(refused.diagnostic, CapacityDiagnostic::Invariant);

            // The projected flow is never accepted under the default strategy.
            let legacy = analyze_with_strategy(&projected, AllocationStrategy::Legacy).unwrap_err();
            assert_eq!(legacy.diagnostic, CapacityDiagnostic::Invariant);
        })
        .unwrap();

    // A source rescale (capacity 4096, loop 32) selects nothing anywhere.
    let rescaled = SOURCE.replacen("while next < 4096", "while next < 32", 1);
    assert_ne!(rescaled, SOURCE);
    assert!(matches!(
        validate_selected(&parsed(&rescaled)),
        Err(SelectedRefusal::Source(_))
    ));
}
