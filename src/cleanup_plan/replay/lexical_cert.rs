//! Authentication of cleanup-plan v18 lexical-control certificates.
//!
//! A certificate is proof data and carries no authority. Every field is
//! recomputed from independently replayed typed HIR (`lexical_hir`, the HIR
//! lexical validators) and from the plan's own blocks, edges and exits. Plan
//! vectors keep their authored runtime order; they are compared, never sorted
//! or repaired. The proof CFG stays acyclic: a loop latch is a terminal exit,
//! and the only edge-free continuation is a generated advancement fragment
//! entered from a certified `continue`.
use super::*;
use crate::ast::LexicalExitKind;
use crate::cleanup_plan::{CleanupPlan, ExitTargetId};
use crate::hir::{ControlTarget, FunctionExecutionId};

/// Facts the flow replay needs after the certificates are authenticated.
pub(super) struct LexicalAuth {
    /// Loop certificate index by the condition expression its target names.
    pub(super) loops: BTreeMap<ExpressionId, usize>,
}

/// Entry of the generated advancement fragment a certified `continue` hands to.
pub(super) fn latch_resume(plan: &CleanupPlan, exit: ExitTargetId) -> Option<BlockId> {
    let control = plan.lexical_control.as_deref()?;
    control
        .exits
        .iter()
        .find(|certificate| certificate.exit == exit)?
        .resume
}

/// Region that owns the loop a target names.
pub(super) fn loop_region(plan: &CleanupPlan, target: &ControlTarget) -> Option<CleanupRegionId> {
    let ControlTarget::Loop { site, .. } = target else {
        return None;
    };
    let control = plan.lexical_control.as_deref()?;
    control
        .loops
        .iter()
        .find(|certificate| &certificate.condition == site)
        .map(|certificate| certificate.region)
}

fn reach(
    function: &ResolvedFunction,
    start: BlockId,
    avoid: Option<BlockId>,
    stop: &BTreeSet<BlockId>,
) -> Vec<bool> {
    let mut seen = vec![false; function.cleanup_plan.blocks.len()];
    if Some(start) == avoid {
        return seen;
    }
    seen[start.0 as usize] = true;
    let mut stack = vec![start];
    while let Some(block) = stack.pop() {
        for next in block_successors(function, block) {
            let index = next.0 as usize;
            if seen[index] || Some(next) == avoid || stop.contains(&next) {
                continue;
            }
            seen[index] = true;
            stack.push(next);
        }
    }
    seen
}

fn chain_contains(
    function: &ResolvedFunction,
    block: BlockId,
    region: CleanupRegionId,
) -> Result<bool, Diagnostic> {
    let start = function.cleanup_plan.blocks[block.0 as usize].region;
    Ok(region_chain_to_root(function, start)?.contains(&region))
}

enum Domain {
    Ordinary,
    Return,
    Latch(usize),
}

pub(super) fn authenticate(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    owner: &FunctionExecutionId,
    facts: &lexical_hir::LexicalFacts<'_>,
    budget: &mut ReplayBudget,
) -> Result<LexicalAuth, Diagnostic> {
    let plan = &function.cleanup_plan;
    let fail = |message: &str| replay_error(function, message.to_owned());
    let control = plan
        .lexical_control
        .as_deref()
        .ok_or_else(|| fail("lexical-profile function has no lexical-control certificate"))?;
    if control.schema != crate::hir::lexical_exit::LEXICAL_CONTROL_FLOW_SCHEMA_V1 {
        return Err(fail("lexical-control certificate uses an unknown schema"));
    }
    // Charge every graph traversal below before the first one runs: two per loop
    // (dominance and reach), one per resume, one ordinary reach, each visiting
    // every block, edge and exit and scanning the exit certificates, plus the
    // per-edge region-chain walks of each abrupt-only region.
    let graph = plan
        .blocks
        .len()
        .saturating_add(plan.edges.len())
        .saturating_add(plan.exits.len());
    let traversals = control
        .loops
        .len()
        .saturating_mul(2)
        .saturating_add(control.resumes.len())
        .saturating_add(1);
    let units = traversals
        .saturating_mul(graph.saturating_mul(control.exits.len().saturating_add(1)))
        .saturating_add(
            control
                .abrupt_only_regions
                .len()
                .saturating_mul(plan.edges.len())
                .saturating_mul(plan.regions.len()),
        )
        .saturating_add(plan.edges.len().saturating_mul(plan.regions.len()));
    budget.charge(function, units, "lexical certificate replay")?;
    let owned_result = type_needs_drop(program, function, &function.return_type)?;

    // Every identity a certificate names must exist before any traversal.
    for certificate in &control.exits {
        if certificate
            .resume
            .is_some_and(|entry| !block_exists(function, entry))
        {
            return Err(fail(
                "lexical exit certificate names an unknown resume block",
            ));
        }
    }
    for certificate in &control.loops {
        let known_region = plan.regions.get(certificate.region.0 as usize).is_some();
        if !known_region
            || !block_exists(function, certificate.condition_entry)
            || !block_exists(function, certificate.after)
            || certificate
                .latch_entry
                .is_some_and(|entry| !block_exists(function, entry))
        {
            return Err(fail("loop certificate names an unknown region or block"));
        }
    }
    for resume in &control.resumes {
        if !block_exists(function, resume.entry)
            || resume
                .blocks
                .iter()
                .any(|block| !block_exists(function, *block))
        {
            return Err(fail("resume certificate names an unknown block"));
        }
    }

    // Loops: independently derived order, target, latch and edge structure.
    if control.loops.len() != facts.loops.len() {
        return Err(fail(
            "loop certificates do not cover exactly the reachable HIR loops",
        ));
    }
    let mut loops = BTreeMap::new();
    for (index, (certificate, fact)) in control.loops.iter().zip(&facts.loops).enumerate() {
        let expected = ControlTarget::Loop {
            owner: owner.clone(),
            site: fact.condition.id.clone(),
        };
        if certificate.target != expected
            || certificate.condition != fact.condition.id
            || certificate.body != fact.body.id
            || &certificate.latch != fact.latch
        {
            return Err(fail("loop certificate disagrees with its typed HIR loop"));
        }
        if loops.insert(certificate.condition.clone(), index).is_some() {
            return Err(fail("loop certificate condition is repeated"));
        }
        let (branch, true_edge, false_edge) = loop_branch(function, &certificate.condition)?;
        if plan.edges[false_edge.0 as usize].to != certificate.after
            || plan.blocks[certificate.after.0 as usize].region != certificate.region
            || plan.blocks[certificate.condition_entry.0 as usize].region != certificate.region
            || plan.blocks[plan.edges[true_edge.0 as usize].to.0 as usize].region
                != certificate.region
        {
            return Err(fail(
                "loop certificate blocks do not match the condition branch and loop region",
            ));
        }
        // The concrete head must dominate the branch and reach it.
        let none = BTreeSet::new();
        if reach(
            function,
            plan.entry,
            Some(certificate.condition_entry),
            &none,
        )[branch.0 as usize]
            || !reach(function, certificate.condition_entry, None, &none)[branch.0 as usize]
        {
            return Err(fail(
                "loop condition entry does not dominate its condition branch",
            ));
        }
    }

    // Exits: independently derived order, site, kind, target and continuation.
    if control.exits.len() != facts.exits.len() {
        return Err(fail(
            "exit certificates do not cover exactly the reachable HIR exits",
        ));
    }
    let mut certified = BTreeSet::new();
    let mut return_entries = Vec::new();
    let mut continues = BTreeMap::<usize, Vec<Option<BlockId>>>::new();
    for (certificate, fact) in control.exits.iter().zip(&facts.exits) {
        if certificate.site != fact.expression.id
            || certificate.kind != fact.kind
            || &certificate.target != fact.target
        {
            return Err(fail("exit certificate disagrees with its typed HIR exit"));
        }
        let exit = plan
            .exits
            .get(certificate.exit.0 as usize)
            .filter(|exit| exit.id == certificate.exit)
            .ok_or_else(|| fail("exit certificate names an unknown exit"))?;
        if !certified.insert(certificate.exit) {
            return Err(fail("exit certificate names a repeated exit"));
        }
        match (fact.kind, &exit.continuation) {
            (
                LexicalExitKind::Return,
                ExitContinuation::FunctionReturn {
                    target,
                    edge,
                    source,
                },
            ) => {
                let operand = fact
                    .value
                    .ok_or_else(|| fail("function return has no operand"))?;
                let expected_target = ControlTarget::Function(owner.clone());
                let source_matches = match source {
                    CleanupResultSource::Scalar { expression } => {
                        !owned_result && expression == &operand.id
                    }
                    CleanupResultSource::Owned { storage } => {
                        owned_result
                            && storage == &CleanupPlace::whole(StorageId::ProvisionalResult)
                    }
                };
                let entry = plan.edges[edge.0 as usize].to;
                if target != &expected_target
                    || certificate.target != expected_target
                    || !source_matches
                    || certificate.resume != Some(entry)
                {
                    return Err(fail(
                        "function return exit disagrees with its operand, result source or resume",
                    ));
                }
                return_entries.push(entry);
            }
            (LexicalExitKind::Break, ExitContinuation::LoopBreak { target, edge }) => {
                let index = loop_index(&loops, owner, &certificate.target)
                    .ok_or_else(|| fail("loop break names an unknown loop"))?;
                if target != &certificate.target
                    || plan.edges[edge.0 as usize].to != control.loops[index].after
                    || certificate.resume.is_some()
                    || !chain_contains(function, exit.from, control.loops[index].region)?
                {
                    return Err(fail("loop break exit disagrees with its loop join"));
                }
            }
            (LexicalExitKind::Continue, ExitContinuation::LoopLatch { target }) => {
                let index = loop_index(&loops, owner, &certificate.target)
                    .ok_or_else(|| fail("loop continue names an unknown loop"))?;
                if target != &certificate.target {
                    return Err(fail("loop continue exit names another target"));
                }
                continues.entry(index).or_default().push(certificate.resume);
            }
            _ => {
                return Err(fail(
                    "exit certificate kind disagrees with its cleanup continuation",
                ));
            }
        }
    }

    // Latch entries and normal iteration ends, per loop.
    let mut iteration_ends = BTreeSet::new();
    let mut latch_entries = BTreeMap::<BlockId, usize>::new();
    for (index, (certificate, fact)) in control.loops.iter().zip(&facts.loops).enumerate() {
        let generated = lexical_hir::generated_advance(fact.body, fact.latch).is_some();
        let resumes = continues.get(&index).map(Vec::as_slice).unwrap_or(&[]);
        match (generated && !resumes.is_empty(), certificate.latch_entry) {
            (true, Some(entry)) => {
                if resumes.iter().any(|resume| *resume != Some(entry))
                    || latch_entries.insert(entry, index).is_some()
                {
                    return Err(fail(
                        "continue exits do not share their loop's generated latch entry",
                    ));
                }
            }
            (false, None) => {
                if resumes.iter().any(Option::is_some) {
                    return Err(fail("ordinary loop continue carries a generated resume"));
                }
            }
            _ => {
                return Err(fail(
                    "loop latch entry disagrees with its generated advancement",
                ));
            }
        }
        if fact.body_completes {
            let exit = plan
                .exits
                .get(certificate.iteration_end.0 as usize)
                .filter(|exit| exit.id == certificate.iteration_end)
                .ok_or_else(|| fail("loop certificate has no normal iteration end"))?;
            if !matches!(
                &exit.continuation,
                ExitContinuation::LoopLatch { target } if target == &certificate.target
            ) || certified.contains(&certificate.iteration_end)
                || !iteration_ends.insert(certificate.iteration_end)
            {
                return Err(fail(
                    "loop iteration end is not its own uncertified latch exit",
                ));
            }
        } else if certificate.iteration_end != UNRESOLVED_NORMAL_END {
            // The sentinel is accepted only for an independently abrupt body.
            return Err(fail(
                "loop with a normal endpoint carries the abrupt-only iteration sentinel",
            ));
        }
    }

    // Resume inventories: exact reconstructed membership, disjoint domains.
    let mut entries = BTreeSet::new();
    for resume in &control.resumes {
        if !entries.insert(resume.entry) {
            return Err(fail("resume certificate entry is repeated"));
        }
    }
    let expected_entries = return_entries
        .iter()
        .copied()
        .chain(latch_entries.keys().copied())
        .collect::<BTreeSet<_>>();
    if entries != expected_entries
        || expected_entries.len() != return_entries.len() + latch_entries.len()
    {
        return Err(fail(
            "resume certificates do not cover exactly the return and latch entries",
        ));
    }
    let ordered_returns = control
        .resumes
        .iter()
        .map(|resume| resume.entry)
        .filter(|entry| return_entries.contains(entry))
        .collect::<Vec<_>>();
    if ordered_returns != return_entries {
        return Err(fail("return resume certificates are not in return order"));
    }
    let block_count = plan.blocks.len();
    let mut domains = vec![None::<usize>; block_count];
    let ordinary = reach(function, plan.entry, None, &entries);
    for (index, resume) in control.resumes.iter().enumerate() {
        let none = BTreeSet::new();
        let seen = reach(function, resume.entry, None, &none);
        let mut inventory = vec![resume.entry];
        inventory.extend(
            seen.iter()
                .enumerate()
                .filter(|(block, seen)| **seen && *block != resume.entry.0 as usize)
                .map(|(block, _)| BlockId(block as u32)),
        );
        if resume.blocks != inventory {
            return Err(fail(
                "resume certificate is not its entry followed by its exact reachable blocks",
            ));
        }
        for block in &inventory {
            let slot = &mut domains[block.0 as usize];
            if ordinary[block.0 as usize] || slot.replace(index).is_some() {
                return Err(fail("lexical proof domains overlap"));
            }
        }
    }
    if (0..block_count).any(|block| !ordinary[block] && domains[block].is_none()) {
        return Err(fail("cleanup block belongs to no lexical proof domain"));
    }

    // Domain entry discipline: only certified exits reach a resume.
    let return_edges = plan
        .exits
        .iter()
        .filter_map(|exit| match &exit.continuation {
            ExitContinuation::FunctionReturn { edge, .. } => Some((*edge, exit.from)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    for edge in &plan.edges {
        let Some(target) = domains[edge.to.0 as usize] else {
            continue;
        };
        let inside = domains[edge.from.0 as usize] == Some(target);
        let certified_entry = control.resumes[target].entry == edge.to
            && return_edges.get(&edge.id) == Some(&edge.from);
        if !inside && !certified_entry {
            return Err(fail(
                "an edge enters a lexical resume outside its certified exit",
            ));
        }
    }

    // Per-domain exit shapes, uncertified latches and success exits.
    let mut ordinary_commits = 0usize;
    let mut resume_commits = vec![0usize; control.resumes.len()];
    let mut resume_latches = vec![0usize; control.resumes.len()];
    let mut commit_exits = Vec::new();
    for exit in &plan.exits {
        let domain = match domains[exit.from.0 as usize] {
            None => Domain::Ordinary,
            Some(index) if return_entries.contains(&control.resumes[index].entry) => Domain::Return,
            Some(index) => Domain::Latch(index),
        };
        let accounted = certified.contains(&exit.id) || iteration_ends.contains(&exit.id);
        match (&domain, &exit.continuation) {
            (_, ExitContinuation::Continue(_) | ExitContinuation::ReturnFailure { .. }) => {}
            (Domain::Ordinary, ExitContinuation::CommitResult { .. }) => {
                ordinary_commits += 1;
                commit_exits.push(exit.id);
            }
            (Domain::Return, ExitContinuation::CommitResult { .. }) => {
                resume_commits[domains[exit.from.0 as usize].expect("return domain")] += 1;
                commit_exits.push(exit.id);
            }
            (
                Domain::Ordinary,
                ExitContinuation::LoopBreak { .. }
                | ExitContinuation::FunctionReturn { .. }
                | ExitContinuation::LoopLatch { .. },
            ) if accounted => {}
            (Domain::Latch(index), ExitContinuation::LoopLatch { target }) if !accounted => {
                let owner_loop = latch_entries
                    .get(&control.resumes[*index].entry)
                    .copied()
                    .ok_or_else(|| fail("latch fragment has no loop"))?;
                if target != &control.loops[owner_loop].target {
                    return Err(fail("latch fragment ends at another loop's latch"));
                }
                resume_latches[*index] += 1;
            }
            _ => {
                return Err(fail(
                    "exit continuation is not admitted in its lexical proof domain",
                ));
            }
        }
    }
    if ordinary_commits != usize::from(facts.body_completes) {
        return Err(fail(
            "ordinary success exits disagree with the body's normal completion",
        ));
    }
    for (index, resume) in control.resumes.iter().enumerate() {
        let is_return = return_entries.contains(&resume.entry);
        if (is_return && (resume_commits[index] != 1 || resume_latches[index] != 0))
            || (!is_return && (resume_latches[index] != 1 || resume_commits[index] != 0))
        {
            return Err(fail(
                "resume proof domain does not end in exactly one terminal exit",
            ));
        }
    }
    if control.function_success_exits != commit_exits
        || !control
            .function_success_exits
            .contains(&plan.regions[0].normal_scope_end)
    {
        return Err(fail(
            "function success exits are not the exact ordinary and return commits",
        ));
    }
    if iteration_ends.len()
        != facts
            .loops
            .iter()
            .filter(|fact| fact.body_completes)
            .count()
    {
        return Err(fail(
            "normal iteration ends are not one per completing loop",
        ));
    }

    validate_normal_ends(function, control)?;
    Ok(LexicalAuth { loops })
}

fn loop_index(
    loops: &BTreeMap<ExpressionId, usize>,
    owner: &FunctionExecutionId,
    target: &ControlTarget,
) -> Option<usize> {
    match target {
        ControlTarget::Loop {
            owner: actual,
            site,
        } if actual == owner => loops.get(site).copied(),
        _ => None,
    }
}

/// The unique `Branch` whose Boolean edges decide the loop condition.
fn loop_branch(
    function: &ResolvedFunction,
    condition: &ExpressionId,
) -> Result<(BlockId, EdgeId, EdgeId), Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut yes = None;
    let mut no = None;
    for edge in &plan.edges {
        match &edge.condition {
            EdgeCondition::BooleanResult(expression, value) if expression == condition => {
                let slot = if *value { &mut yes } else { &mut no };
                if slot.replace(edge.id).is_some() {
                    return Err(replay_error(
                        function,
                        "loop condition has repeated Boolean edges",
                    ));
                }
            }
            _ => {}
        }
    }
    match (yes, no) {
        (Some(yes), Some(no))
            if plan.edges[yes.0 as usize].from == plan.edges[no.0 as usize].from =>
        {
            Ok((plan.edges[yes.0 as usize].from, yes, no))
        }
        _ => Err(replay_error(
            function,
            "loop condition has no complementary Boolean branch",
        )),
    }
}

/// Normal scope ends are real scope exits; a provisional end must be an
/// abrupt-only region whose subtree has no normal continuation or escape.
fn validate_normal_ends(
    function: &ResolvedFunction,
    control: &crate::cleanup_plan::LexicalControlPlan,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let fail = |message: &str| replay_error(function, message.to_owned());
    let lexical_edges = plan
        .exits
        .iter()
        .filter_map(|exit| match &exit.continuation {
            ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => Some(*edge),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut listed = BTreeSet::new();
    for region in &control.abrupt_only_regions {
        if region.0 == 0 || plan.regions.get(region.0 as usize).is_none() || !listed.insert(*region)
        {
            return Err(fail("abrupt-only region list is invalid or repeated"));
        }
    }
    for region in plan.regions.iter().skip(1) {
        let unresolved = region.normal_scope_end == UNRESOLVED_NORMAL_END;
        if unresolved != listed.contains(&region.id) {
            return Err(fail(
                "provisional normal scope end differs from the abrupt-only certificate",
            ));
        }
        if !unresolved {
            let exit = &plan.exits[region.normal_scope_end.0 as usize];
            if !matches!(exit.continuation, ExitContinuation::Continue(_))
                || exit.leaves_regions != [region.id]
            {
                return Err(fail("region normal scope end is not its own scope exit"));
            }
            continue;
        }
        for exit in &plan.exits {
            if matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit.leaves_regions == [region.id]
            {
                return Err(fail("abrupt-only region has a normal scope exit"));
            }
        }
        for edge in &plan.edges {
            if chain_contains(function, edge.from, region.id)?
                && !chain_contains(function, edge.to, region.id)?
                && !lexical_edges.contains(&edge.id)
            {
                return Err(fail(
                    "abrupt-only region flow escapes without a lexical exit",
                ));
            }
        }
    }
    if plan.regions[0].normal_scope_end == UNRESOLVED_NORMAL_END {
        return Err(fail("root region has no normal success exit"));
    }
    Ok(())
}
