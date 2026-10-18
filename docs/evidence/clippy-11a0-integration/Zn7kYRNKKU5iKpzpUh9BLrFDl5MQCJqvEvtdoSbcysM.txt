//! Complete actual-current selected-payer controls, separate from the OPT876 marginal claim.
use super::*;

fn observe_live(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    roots: &[usize],
    old: bool,
    limit: usize,
) -> (Outcome, Metrics) {
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(budget) => budget,
            Err(error) => {
                return Outcome {
                    result: Err(Fields::capture(error)),
                    sticky: None,
                }
            }
        };
        let result = (|| {
            budget.reserve_bytes(
                function,
                Canonicalizer::CONSTRUCTOR_CONTROLS,
                "canonical ordered dictionary owner receiving",
            )?;
            let mut owner = Canonicalizer::new(graph, sink);
            let mut checksum = 0u64;
            for root in roots {
                budget.reserve_array::<(usize, u64)>(function, 1, "arena corpus root")?;
                let id = if old {
                    owner.before_id(function, vec![(*root, 1)], &mut budget)
                } else {
                    owner.id(function, vec![(*root, 1)], &mut budget)
                }?
                .unwrap();
                checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
            }
            Ok::<_, Diagnostic>(Stats {
                paid: budget.physical.as_ref().unwrap().charged(),
                work: (
                    budget.remaining,
                    budget.skeleton_remaining,
                    budget.skeleton_spent,
                ),
                checksum,
                counts: (owner.ids.len(), owner.states.len()),
            })
        })();
        let sticky = if result.is_err() {
            let bytes = budget.physical.as_ref().unwrap().charged();
            let work = (
                budget.remaining,
                budget.skeleton_remaining,
                budget.skeleton_spent,
            );
            let allocated = current_metrics().allocated;
            let retry = budget
                .reserve_bytes(function, 0, "lexical child scratch sticky retry")
                .unwrap_err();
            assert_eq!(budget.physical.as_ref().unwrap().charged(), bytes);
            assert_eq!(
                (
                    budget.remaining,
                    budget.skeleton_remaining,
                    budget.skeleton_spent
                ),
                work
            );
            assert_eq!(current_metrics().allocated, allocated);
            Some(Fields::capture(retry))
        } else {
            None
        };
        drop(budget);
        Outcome {
            result: result.map_err(Fields::capture),
            sticky,
        }
    })
}

#[test]
fn actual_current_selected_route_has_exact_later_arity_cost_without_repricing_876() {
    let function = fixture();
    for shape in 0..3 {
        let (graph, sink, roots) = corpus(&function, shape);
        let (old, old_heap) = observe_live(&function, &graph, sink, &roots, true, 1 << 26);
        let (live, live_heap) = observe_live(&function, &graph, sink, &roots, false, 1 << 26);
        let (matched, matched_heap) = observe(&function, &graph, sink, &roots, false, 1 << 26);
        let old = old.result.unwrap();
        let live = live.result.unwrap();
        let matched = matched.result.unwrap();
        assert_eq!(
            (live.work, live.checksum, live.counts),
            (old.work, old.checksum, old.counts)
        );
        assert_eq!(
            (live.work, live.checksum, live.counts),
            (matched.work, matched.checksum, matched.counts)
        );
        assert_eq!(
            live_heap, old_heap,
            "complete allocation, peak and Drop, no backing removed by arity routing"
        );
        assert_eq!(live_heap, matched_heap);
        assert!(!live_heap.invalid && live_heap.live == 0);
        assert_eq!(live_heap.allocated, live_heap.freed);
        // Independent source/corpus counts. Each nonempty input is distinct;
        // its one nonrecursive frame activation has 0 or >=24 outgoing labels.
        // The terminal-only input takes cold_dead_id and never forms that frame.
        let activations = roots
            .iter()
            .filter(|root| !graph.nodes[**root].moves().is_empty())
            .count();
        assert!(graph.nodes.iter().all(|node| node.moves().len() != 1));
        let receipts = activations * std::mem::size_of::<Result<(), Diagnostic>>();
        let later = activations * Canonicalizer::BORROWED_EXIT_LOCAL;
        assert_eq!(
            Canonicalizer::BORROWED_EXIT_LOCAL,
            2 * std::mem::size_of::<usize>() + 4 * std::mem::size_of::<bool>()
        );
        assert_eq!(live.paid, matched.paid + later,
            "actual later retained arity receiving places are additional, never credited to the old bank");
        assert_eq!(live.paid + receipts, old.paid + later,
            "complete typed role equation uses addition; no assumption that arbitrary composed routes save bytes");
        for old_route in [true, false] {
            let full = if old_route { old.paid } else { live.paid };
            for limit in [0, full / 2, full - 1, full] {
                let first = observe_live(&function, &graph, sink, &roots, old_route, limit);
                let again = observe_live(&function, &graph, sink, &roots, old_route, limit);
                assert_eq!(
                    first, again,
                    "all six fields, sticky work, heap and whole Drop at its own real boundary"
                );
                assert_eq!(first.0.result.is_ok(), limit == full);
                assert!(!first.1.invalid && first.1.live == 0);
                assert_eq!(first.1.allocated, first.1.freed);
                if let Err(error) = first.0.result {
                    assert_eq!(error.code, "SPX-H006");
                }
            }
        }
        eprintln!("OPT876 composed selected shape={shape} old/matched/live={}/{}/{} receipts={receipts} later={later} heap={live_heap:?}", old.paid, matched.paid, live.paid);
    }
}
