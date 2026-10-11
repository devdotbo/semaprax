//! Factored typed-control skeleton comparison.
//!
//! The enumerating comparison materializes every terminal path of the typed
//! HIR and of the cleanup CFG and compares the sorted multisets. Independent
//! decisions in sequence multiply that count (2^N for N independent `if`s,
//! `&&` operands, or scalar `match`es), although a decision that leaves the
//! cleanup state unchanged contributes nothing a single path would not.
//!
//! This module decides the same multiset equality without enumerating the
//! product:
//!
//! * The typed-HIR skeleton is derived by the unchanged walk with path-state
//!   merging enabled: whenever paths are sequenced, paths whose cleanup state
//!   (owned source, failure, residual, and call commit list) is identical are
//!   merged into one path carrying the union of their observation sequences.
//!   Every later skeleton step reads only that state, so the merged walk
//!   denotes exactly the enumerated paths, and its size follows the number of
//!   distinct cleanup states rather than the number of decision combinations.
//!   Lexical functions restrict those merges to independently identified Copy
//!   expression scopes; every abrupt/failure/owning state remains separate.
//! * Both the merged HIR paths and the cleanup CFG become acyclic automata
//!   over skeleton observations, each accepting sequence ending in one
//!   terminal label.
//! * Bottom-up hash-consing of the weighted subset construction assigns two
//!   states the same id exactly when they accept every sequence the same
//!   number of times, so equal roots prove equal path multisets.
use super::skeleton_work::{ObservationDag, ObservationDagKind};
use super::*;
use std::rc::Rc;
pub(super) mod copy_regions;
mod storage;
use storage::{memo_insert, push_end, push_frame, reserve_transition_observation};

/// Path count above which the factored comparison replaces enumeration.
const FACTORED_PATH_THRESHOLD: usize = 4_096;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Label {
    Observation(Rc<SkeletonObservation>),
    End(SkeletonTerminal),
}

#[derive(Default)]
struct Node {
    moves: Vec<(Label, usize)>,
    epsilon: Vec<usize>,
}

/// `true` when this function's typed-control skeleton is compared factored.
/// A cyclic CFG saturates the path census and keeps the census diagnostic.
pub(super) fn selected(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
) -> bool {
    (cfg_paths > FACTORED_PATH_THRESHOLD || semantic_paths > FACTORED_PATH_THRESHOLD)
        && validate_reachable_acyclic_cfg(function).is_ok()
}

/// Run the factored comparison when it is selected. `Ok(true)` means the
/// skeleton is authenticated; `Ok(false)` leaves the enumerating comparison
/// in charge.
#[cfg(test)]
pub(super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    validate_with_index(program, function, budget, None)
}
pub(super) fn validate_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<bool, Diagnostic> {
    let cfg_paths = branch_sensitive_cfg_bounds(function)?.terminal_paths;
    let semantic_paths = hir_terminal_path_bound(function)?;
    if !selected(function, cfg_paths, semantic_paths) {
        #[cfg(test)]
        cross_check(program, function);
        return Ok(false);
    }
    match compare_with_index(program, function, budget, helpers) {
        Ok(Some(true)) => Ok(true),
        Ok(Some(false)) => Err(skeleton_mismatch(function)),
        verdict => {
            if cfg_paths <= MAX_REPLAY_PATHS && semantic_paths <= MAX_REPLAY_PATHS {
                // Enumeration still fits; an attempted factored comparison
                // keeps every work and physical debit already spent.
                return Ok(false);
            }
            match verdict {
                Err(error) if !error.message.contains("budget") => Err(error),
                _ => path_summary::path_budget_errors(function, cfg_paths, semantic_paths)
                    .map(|()| false),
            }
        }
    }
}

fn skeleton_mismatch(function: &ResolvedFunction) -> Diagnostic {
    replay_error(
        function,
        "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
    )
}

/// `Some(equal)`, or `None` when a path multiplicity overflows.
#[cfg(test)]
pub(super) fn compare(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Diagnostic> {
    compare_with_index(program, function, budget, None)
}
fn compare_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<Option<bool>, Diagnostic> {
    let BuiltComparison {
        automaton,
        sink,
        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers)?;

    let mut canonical = Canonicalizer::new(&automaton, sink);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
    let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
        return Ok(None);
    };
    budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
    let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
        return Ok(None);
    };
    Ok(Some(hir == plan))
}

struct BuiltComparison {
    automaton: Automaton,
    sink: usize,
    hir_root: usize,
    plan_root: usize,
}

/// Reconstruct both exact observation languages once, retaining every debit.
/// The exact strategy census borrows this same physical owner; it never
/// retries construction after choosing a representation.
fn build_comparison(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<BuiltComparison, Diagnostic> {
    // Both independent constructors debit skeleton helpers as well as ordinary
    // constructors. Keep one active allowance through the HIR walk AND plan
    // automaton construction; the held program reservation belongs to other
    // enumerated functions and cannot pay for this deferred attempt.
    let skeleton_reserve = budget.skeleton_remaining;
    budget.skeleton_remaining = std::mem::take(&mut budget.remaining);
    budget.lent_skeleton = true;
    let result = (|| {
        budget.merge_paths = true;
        let hir_paths = hir_skeleton_paths_with_index(program, function, budget, helpers);
        budget.merge_paths = false;
        let hir_paths = hir_paths?;

        let mut automaton = Automaton::default();
        let sink = automaton.node(function, budget)?;
        let success = automaton.end(function, budget, sink, SkeletonTerminal::Success)?;
        let failure = automaton.end(function, budget, sink, SkeletonTerminal::Failure)?;
        let latch = automaton.end(function, budget, sink, SkeletonTerminal::LoopLatch)?;
        let hir_root = automaton.node(function, budget)?;
        let mut memo = BTreeMap::new();
        for path in &hir_paths {
            let end = automaton.add_observations(
                function,
                &path.observations,
                hir_root,
                &mut memo,
                budget,
            )?;
            let terminal = match path.terminal {
                SkeletonTerminal::Success => success,
                SkeletonTerminal::Failure => failure,
                SkeletonTerminal::LoopLatch => latch,
            };
            automaton.epsilon(function, budget, end, terminal)?;
        }
        let plan_root = automaton.add_plan(function, success, failure, latch, budget)?;
        Ok(BuiltComparison {
            automaton,
            sink,
            hir_root,
            plan_root,
        })
    })();
    // Restore only the unused allowance. Successful debits from either
    // constructor remain spent, including an attempt ending in refusal.
    budget.merge_paths = false;
    budget.lent_skeleton = false;
    budget.remaining = std::mem::replace(&mut budget.skeleton_remaining, skeleton_reserve);
    result
}

#[path = "factored/strategy.rs"]
pub(super) mod strategy;

#[derive(Default)]
struct Automaton {
    nodes: Vec<Node>,
}

enum BuildFrame<'d> {
    Visit(&'d Rc<ObservationDag>, usize),
    Seq(&'d Rc<ObservationDag>, usize),
    Cat(&'d Rc<ObservationDag>, usize, usize),
    Alt(&'d Rc<ObservationDag>, usize, usize),
}

impl Automaton {
    fn node(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        budget.charge(function, 1, "factored automaton node construction")?;
        budget.reserve_vec(function, &mut self.nodes, 1, "factored automaton nodes")?;
        self.nodes.push(Node::default());
        Ok(self.nodes.len() - 1)
    }

    fn movement(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        from: usize,
        label: Label,
        to: usize,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "factored automaton labelled movement")?;
        budget.reserve_vec(
            function,
            &mut self.nodes[from].moves,
            1,
            "factored automaton moves",
        )?;
        self.nodes[from].moves.push((label, to));
        Ok(())
    }

    fn epsilon(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        from: usize,
        to: usize,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "factored automaton epsilon movement")?;
        budget.reserve_vec(
            function,
            &mut self.nodes[from].epsilon,
            1,
            "factored automaton epsilon moves",
        )?;
        self.nodes[from].epsilon.push(to);
        Ok(())
    }

    fn observation(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        from: usize,
        observation: SkeletonObservation,
        to: usize,
    ) -> Result<(), Diagnostic> {
        budget.reserve_bytes(
            function,
            physical_clone::observation_rc_bytes(),
            "factored automaton observation header",
        )?;
        self.movement(
            function,
            budget,
            from,
            Label::Observation(Rc::new(observation)),
            to,
        )
    }

    fn end(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        sink: usize,
        terminal: SkeletonTerminal,
    ) -> Result<usize, Diagnostic> {
        let node = self.node(function, budget)?;
        self.movement(function, budget, node, Label::End(terminal), sink)?;
        Ok(node)
    }

    fn chain(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        mut end: usize,
        items: &[Rc<SkeletonObservation>],
    ) -> Result<usize, Diagnostic> {
        for item in items {
            let next = self.node(function, budget)?;
            self.movement(
                function,
                budget,
                end,
                Label::Observation(item.clone()),
                next,
            )?;
            end = next;
        }
        Ok(end)
    }

    /// Build `observations` forward from `start` and return its end node.
    /// A shared DAG node is built once per start node, so a merged prefix
    /// that many later paths extend exists once in the automaton.
    fn add_observations(
        &mut self,
        function: &ResolvedFunction,
        observations: &Observations,
        start: usize,
        memo: &mut BTreeMap<(usize, usize), usize>,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let (prefix, items) = observations.parts();
        let mut end = start;
        if let Some(prefix) = prefix {
            end = self.add_dag(function, prefix, start, memo, budget)?;
        }
        budget.charge(
            function,
            items.len().saturating_add(1),
            "factored HIR automaton",
        )?;
        self.chain(function, budget, end, items)
    }

    fn add_dag(
        &mut self,
        function: &ResolvedFunction,
        root: &Rc<ObservationDag>,
        start: usize,
        memo: &mut BTreeMap<(usize, usize), usize>,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let key = |dag: &Rc<ObservationDag>, start: usize| (Rc::as_ptr(dag) as usize, start);
        budget.reserve_array::<BuildFrame<'_>>(function, 1, "factored DAG initial frame")?;
        let mut frames = vec![BuildFrame::Visit(root, start)];
        let mut ends = Vec::<usize>::new();
        while let Some(frame) = frames.pop() {
            match frame {
                BuildFrame::Visit(dag, start) => {
                    budget.charge(function, 1, "factored HIR automaton")?;
                    if let Some(end) = memo.get(&key(dag, start)) {
                        push_end(function, budget, &mut ends, *end)?;
                        continue;
                    }
                    match &dag.kind {
                        ObservationDagKind::Seq(prefix, _) => {
                            push_frame(function, budget, &mut frames, BuildFrame::Seq(dag, start))?;
                            match prefix {
                                Some(prefix) => push_frame(
                                    function,
                                    budget,
                                    &mut frames,
                                    BuildFrame::Visit(prefix, start),
                                )?,
                                None => push_end(function, budget, &mut ends, start)?,
                            }
                        }
                        ObservationDagKind::Cat(_) => {
                            push_end(function, budget, &mut ends, start)?;
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Cat(dag, start, 0),
                            )?;
                        }
                        ObservationDagKind::Alt(members) => {
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Alt(dag, start, members.len()),
                            )?;
                            for member in members {
                                push_frame(
                                    function,
                                    budget,
                                    &mut frames,
                                    BuildFrame::Visit(member, start),
                                )?;
                            }
                        }
                    }
                }
                BuildFrame::Seq(dag, start) => {
                    let ObservationDagKind::Seq(_, items) = &dag.kind else {
                        unreachable!("sequence frame holds a sequence node");
                    };
                    let prefix_end = ends.pop().expect("sequence prefix end retained");
                    budget.charge(function, items.len(), "factored HIR automaton")?;
                    let end = self.chain(function, budget, prefix_end, items)?;
                    memo_insert(function, budget, memo, key(dag, start), end)?;
                    push_end(function, budget, &mut ends, end)?;
                }
                BuildFrame::Cat(dag, start, index) => {
                    let ObservationDagKind::Cat(parts) = &dag.kind else {
                        unreachable!("concatenation frame holds a concatenation node");
                    };
                    let end = ends.pop().expect("concatenation part end retained");
                    match parts.get(index) {
                        Some(part) => {
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Cat(dag, start, index + 1),
                            )?;
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Visit(part, end),
                            )?;
                        }
                        None => {
                            memo_insert(function, budget, memo, key(dag, start), end)?;
                            push_end(function, budget, &mut ends, end)?;
                        }
                    }
                }
                BuildFrame::Alt(dag, start, count) => {
                    let join = self.node(function, budget)?;
                    for _ in 0..count {
                        let end = ends.pop().expect("union member end retained");
                        self.epsilon(function, budget, end, join)?;
                    }
                    memo_insert(function, budget, memo, key(dag, start), join)?;
                    push_end(function, budget, &mut ends, join)?;
                }
            }
        }
        Ok(ends.pop().expect("factored DAG build produced an end"))
    }

    fn add_plan(
        &mut self,
        function: &ResolvedFunction,
        success: usize,
        failure: usize,
        latch: usize,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let plan = &function.cleanup_plan;
        budget.charge(
            function,
            plan_structure_units(plan),
            "factored cleanup-plan automaton",
        )?;
        budget.reserve_array::<usize>(
            function,
            plan.blocks.len(),
            "factored plan block entries",
        )?;
        let mut entries = Vec::with_capacity(plan.blocks.len());
        for _ in &plan.blocks {
            entries.push(self.node(function, budget)?);
        }
        let entry = |block: BlockId| entries[block.0 as usize];
        for (index, block) in plan.blocks.iter().enumerate() {
            let mut node = entries[index];
            for transition in &block.transitions {
                if let Some(observation) = transition_observation(function, budget, transition)? {
                    let next = self.node(function, budget)?;
                    self.observation(function, budget, node, observation, next)?;
                    node = next;
                }
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    let edge = &plan.edges[edge.0 as usize];
                    let to = entry(edge.to);
                    if let EdgeCondition::VariantCase { .. } = &edge.condition {
                        let observation = edge_observation(function, budget, &edge.condition)?;
                        self.observation(function, budget, node, observation, to)?;
                    } else {
                        self.epsilon(function, budget, node, to)?;
                    }
                }
                CleanupTerminator::Branch(edges) => {
                    for edge in edges {
                        let edge = &plan.edges[edge.0 as usize];
                        let observation = edge_observation(function, budget, &edge.condition)?;
                        self.observation(function, budget, node, observation, entry(edge.to))?;
                    }
                }
                CleanupTerminator::Exit(exit) => {
                    let exit = &plan.exits[exit.0 as usize];
                    let target = match &exit.continuation {
                        ExitContinuation::Continue(edge) => entry(plan.edges[edge.0 as usize].to),
                        ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                            success
                        }
                        ExitContinuation::ReturnFailure { .. } => failure,
                        ExitContinuation::LoopBreak { edge, .. }
                        | ExitContinuation::FunctionReturn { edge, .. } => {
                            let marker =
                                lexical_skeleton::plan_exit_observation(budget, function, exit.id)?
                                    .ok_or_else(|| {
                                        replay_error(
                                    function,
                                    "factored lexical exit lacks an exact certificate marker",
                                )
                                    })?;
                            // The marker itself is the labelled continuation edge.
                            // A lexical exit must never become an epsilon edge.
                            self.observation(
                                function,
                                budget,
                                node,
                                marker,
                                entry(plan.edges[edge.0 as usize].to),
                            )?;
                            continue;
                        }
                        ExitContinuation::LoopLatch { target } => {
                            let marker = match lexical_skeleton::plan_exit_observation(
                                budget, function, exit.id,
                            )? {
                                Some(marker) => marker,
                                None => SkeletonObservation::LoopLatch {
                                    target: skeleton_clone(
                                        budget,
                                        function,
                                        target,
                                        "factored loop-latch target clone",
                                    )?,
                                },
                            };
                            let target =
                                lexical_cert::latch_resume(plan, exit.id).map_or(latch, entry);
                            self.observation(function, budget, node, marker, target)?;
                            continue;
                        }
                    };
                    self.epsilon(function, budget, node, target)?;
                }
            }
        }
        Ok(entry(plan.entry))
    }
}

/// The skeleton observation one cleanup transition contributes, exactly as
/// the enumerating `plan_skeleton_paths` records it.
fn transition_observation(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    transition: &CleanupTransition,
) -> Result<Option<SkeletonObservation>, Diagnostic> {
    reserve_transition_observation(function, budget, transition)?;
    Ok(Some(match transition {
        CleanupTransition::Initialize { at, destination }
        | CleanupTransition::InitializeVariant {
            at, destination, ..
        } => SkeletonObservation::Initialize {
            at: at.clone(),
            destination: destination.clone(),
        },
        CleanupTransition::ReserveRenewal { at, binding } => SkeletonObservation::ReserveRenewal {
            at: at.clone(),
            binding: binding.clone(),
        },
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        }
        | CleanupTransition::Renew {
            at,
            source,
            destination,
        }
        | CleanupTransition::TransferVariant {
            at,
            source,
            destination,
            ..
        } => SkeletonObservation::Transfer {
            at: at.clone(),
            source: source.clone(),
            destination: destination.clone(),
        },
        CleanupTransition::CallCommit { call, arguments } => SkeletonObservation::CallCommit {
            call: call.clone(),
            arguments: arguments
                .iter()
                .map(|argument| (argument.parameter_index, argument.source.clone()))
                .collect(),
        },
        CleanupTransition::StageCopyResult { source } => {
            SkeletonObservation::StageCopyResult(source.clone())
        }
        CleanupTransition::AuthenticateVariantCase { .. }
        | CleanupTransition::SelectFailure { .. } => return Ok(None),
    }))
}

/// The skeleton observation one edge condition contributes, exactly as the
/// enumerating `plan_skeleton_paths` records it.
fn edge_observation(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    condition: &EdgeCondition,
) -> Result<SkeletonObservation, Diagnostic> {
    if let EdgeCondition::VariantCase { case, .. } = condition {
        budget.reserve_clone(function, case, "factored edge case identity")?;
    }
    Ok(match condition {
        EdgeCondition::BooleanResult(expression, value) => SkeletonObservation::Boolean {
            expression: expression.clone(),
            value: *value,
        },
        EdgeCondition::VariantCase {
            scrutinee,
            case,
            matches,
        } => SkeletonObservation::VariantCase {
            scrutinee: scrutinee.clone(),
            case: case.clone(),
            matches: *matches,
        },
        EdgeCondition::ArmSelected {
            scrutinee,
            arm,
            selected,
        } => SkeletonObservation::ArmSelected {
            scrutinee: scrutinee.clone(),
            arm: *arm,
            selected: *selected,
        },
        EdgeCondition::StatusZero(source) => SkeletonObservation::Status {
            source: source.clone(),
            success: true,
        },
        EdgeCondition::StatusNonzero(source) => SkeletonObservation::Status {
            source: source.clone(),
            success: false,
        },
        EdgeCondition::Always => {
            return Err(replay_error(
                function,
                "branch skeleton contains an unconditional edge",
            ));
        }
    })
}

/// A weighted set of automaton nodes: each node with the number of distinct
/// automaton paths that reach it on one observation sequence.
type Weighted = Vec<(usize, u64)>;

enum CanonicalFrame {
    Enter(Weighted),
    Exit(Weighted, u64, Vec<(Label, Weighted)>),
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: BTreeMap<usize, Rc<BTreeMap<usize, u64>>>,
    states: BTreeMap<Weighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
}

impl<'a> Canonicalizer<'a> {
    fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: BTreeMap::new(),
            states: BTreeMap::new(),
            ids: BTreeMap::new(),
        }
    }

    /// Nodes with moves (or the sink) reachable through epsilon edges, each
    /// with its number of epsilon routes.
    fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic> {
        budget.reserve_array::<(usize, bool)>(function, 1, "factored closure initial cursor")?;
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            budget.charge(function, 1, "factored closure cursor")?;
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                stack.push((node, true));
                for target in &state.epsilon {
                    if !self.closures.contains_key(target) {
                        budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                        stack.push((*target, false));
                    }
                }
                continue;
            }
            let mut closure = BTreeMap::new();
            if !state.moves.is_empty() || node == self.sink {
                budget.reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                closure.insert(node, 1_u64);
            }
            for target in &state.epsilon {
                let Some(inner) = self.closures.get(target) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                for (inner_node, routes) in inner.iter() {
                    budget.charge(function, 1, "factored closure route addition")?;
                    if !closure.contains_key(inner_node) {
                        budget
                            .reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                    }
                    let slot = closure.entry(*inner_node).or_insert(0_u64);
                    let Some(sum) = slot.checked_add(*routes) else {
                        return Ok(None);
                    };
                    *slot = sum;
                }
            }
            budget.charge(
                function,
                closure.len().saturating_add(1),
                "factored skeleton closure",
            )?;
            budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<BTreeMap<usize, u64>>(),
                "factored closure shared header",
            )?;
            budget.reserve_btree_node::<usize, Rc<BTreeMap<usize, u64>>>(
                function,
                "factored closure memo node",
            )?;
            self.closures.insert(node, Rc::new(closure));
        }
        Ok(self.closures.get(&root).cloned())
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, Weighted)>)>, Diagnostic> {
        let mut closed = BTreeMap::<usize, u64>::new();
        for (node, weight) in state {
            let Some(closure) = self.closure(function, *node, budget)? else {
                return Ok(None);
            };
            for (inner, routes) in closure.iter() {
                let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                    closed.get(inner).copied().unwrap_or(0).checked_add(product)
                }) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                if !closed.contains_key(inner) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored weighted closure node",
                    )?;
                }
                closed.insert(*inner, sum);
            }
        }
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        let mut successors = BTreeMap::<Label, BTreeMap<usize, u64>>::new();
        for (node, weight) in &closed {
            let moves = &self.automaton.nodes[*node].moves;
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, BTreeMap<usize, u64>>(
                        function,
                        "factored successor label node",
                    )?;
                }
                let targets = successors.entry(label.clone()).or_default();
                if !targets.contains_key(child) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored successor target node",
                    )?;
                }
                let slot = targets.entry(*child).or_insert(0);
                let Some(sum) = slot.checked_add(*weight) else {
                    return Ok(None);
                };
                *slot = sum;
            }
        }
        budget.reserve_array::<(Label, Weighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        for (label, targets) in successors {
            budget.reserve_array::<(usize, u64)>(
                function,
                targets.len(),
                "factored weighted successor vector",
            )?;
            result.push((label, targets.into_iter().collect()));
        }
        Ok(Some((accepted, result)))
    }

    fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_array::<CanonicalFrame>(function, 1, "factored canonical initial frame")?;
        let root_copy = storage::clone_weighted(function, budget, &root)?;
        let mut frames = vec![CanonicalFrame::Enter(root_copy)];
        let mut open = BTreeSet::new();
        while let Some(frame) = frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if open.contains(&state) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    budget.reserve_btree_node::<Weighted, ()>(
                        function,
                        "factored open-state identity node",
                    )?;
                    open.insert(storage::clone_weighted(function, budget, &state)?);
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    budget.reserve_vec(
                        function,
                        &mut frames,
                        1,
                        "factored canonical exit frame",
                    )?;
                    let exit_index = frames.len();
                    frames.push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &frames[exit_index] else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child =
                            storage::clone_weighted(function, budget, &successors[index].1)?;
                        budget.reserve_vec(
                            function,
                            &mut frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    open.remove(&state);
                    budget.charge(
                        function,
                        successors.len(),
                        "factored canonical signature construction",
                    )?;
                    budget.reserve_array::<(Label, usize)>(
                        function,
                        successors.len(),
                        "factored canonical signature vector",
                    )?;
                    let mut key = Vec::with_capacity(successors.len());
                    for (label, child) in successors {
                        let Some(id) = self.states.get(&child) else {
                            return Err(replay_error(function, "cleanup CFG contains a cycle"));
                        };
                        key.push((label, *id));
                    }
                    let key = (accepted, key);
                    let id = match self.ids.get(&key) {
                        Some(id) => *id,
                        None => {
                            budget.reserve_btree_node::<(u64, Vec<(Label, usize)>), usize>(
                                function,
                                "factored canonical signature node",
                            )?;
                            let id = self.ids.len();
                            self.ids.insert(key, id);
                            id
                        }
                    };
                    if !self.states.contains_key(&state) {
                        budget.reserve_btree_node::<Weighted, usize>(
                            function,
                            "factored canonical state memo node",
                        )?;
                    }
                    self.states.insert(state, id);
                }
            }
        }
        Ok(self.states.get(&root).copied())
    }
}

/// Test builds compare the factored verdict with the enumerating one on every
/// function small enough to enumerate.
#[cfg(test)]
fn cross_check(program: &dyn crate::cleanup::ReplayContext, function: &ResolvedFunction) {
    let saved = SKELETON_MATERIALIZATIONS.with(Cell::get);
    let mut enumerating = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {
        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
        hir.sort();
        plan.sort();
        Ok(hir == plan)
    });
    if let Ok(true) = enumerated {
        let mut factored = ReplayBudget::with_skeleton_limit(0);
        let verdict = compare(program, function, &mut factored);
        assert!(
            matches!(verdict, Ok(Some(true))),
            "factored skeleton comparison disagrees with enumeration for `{}`: {:?}",
            function.id,
            verdict.map_err(|error| error.message)
        );
    }
    SKELETON_MATERIALIZATIONS.with(|count| count.set(saved));
}

#[cfg(test)]
#[path = "factored/lexical_plan_tests.rs"]
mod lexical_plan_tests;
