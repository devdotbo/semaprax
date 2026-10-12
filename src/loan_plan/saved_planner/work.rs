//! Saved original checked-work traversal over the nested adjacency tables.
//!
//! The intrusive move-to-front `PointQueue`, `Reachability`, `live_nodes` and
//! `edge_liveness` are the original bodies. The production byte-prebound
//! constants stay with the production module; they are accounting data about
//! the production layout, not planner behavior.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::loan_plan::{charge, LoanId, WorkCounter};

use super::Cfg;

pub(in crate::loan_plan) type EdgeLiveness = (Vec<Vec<LoanId>>, Vec<Vec<u16>>);

pub(in crate::loan_plan) struct Reachability {
    by_point: Box<[u64]>,
    words: usize,
}

// A fixed intrusive stack has one link pair and membership byte per point.
// Changed successors move to the front even when already pending: on a long
// straight-line CFG this merges earlier starts before advancing to later ones,
// rather than repeatedly chasing staggered seeds with a FIFO wavefront.
struct PointQueue {
    next: Box<[u16]>,
    previous: Box<[u16]>,
    queued: Box<[bool]>,
    head: u16,
}

impl PointQueue {
    const NONE: u16 = u16::MAX;

    fn new(points: usize) -> Self {
        Self {
            next: vec![Self::NONE; points].into_boxed_slice(),
            previous: vec![Self::NONE; points].into_boxed_slice(),
            queued: vec![false; points].into_boxed_slice(),
            head: Self::NONE,
        }
    }

    fn push(&mut self, point: u16) {
        if self.head == point {
            return;
        }
        if self.queued[point as usize] {
            let previous = self.previous[point as usize];
            let next = self.next[point as usize];
            self.next[previous as usize] = next;
            if next != Self::NONE {
                self.previous[next as usize] = previous;
            }
        }
        self.previous[point as usize] = Self::NONE;
        self.next[point as usize] = self.head;
        if self.head != Self::NONE {
            self.previous[self.head as usize] = point;
        }
        self.head = point;
        self.queued[point as usize] = true;
    }

    fn pop(&mut self) -> Option<u16> {
        if self.head == Self::NONE {
            return None;
        }
        let point = self.head;
        self.head = self.next[point as usize];
        if self.head != Self::NONE {
            self.previous[self.head as usize] = Self::NONE;
        }
        self.queued[point as usize] = false;
        Some(point)
    }
}

impl Reachability {
    pub(in crate::loan_plan) fn build(
        cfg: &Cfg<'_>,
        starts: impl ExactSizeIterator<Item = u16> + DoubleEndedIterator,
        work: &mut WorkCounter,
    ) -> Result<Self, Diagnostic> {
        let words = starts.len().div_ceil(u64::BITS as usize);
        let mut by_point = vec![0; words * cfg.points.len()].into_boxed_slice();
        let mut pending = PointQueue::new(cfg.points.len());
        // Charge actual row initialization, seeding, dequeues and edge-word
        // merges. No hypothetical separate traversal per loan is charged.
        for _ in &by_point {
            charge(work)?;
        }
        for (loan, start) in starts.enumerate().rev() {
            charge(work)?;
            by_point[start as usize * words + loan / 64] |= 1u64 << (loan % 64);
            pending.push(start);
        }
        while let Some(from) = pending.pop() {
            charge(work)?;
            for to in cfg.successors[from as usize].iter().rev() {
                let mut changed = false;
                for word in 0..words {
                    charge(work)?;
                    let before = by_point[*to as usize * words + word];
                    let after = before | by_point[from as usize * words + word];
                    by_point[*to as usize * words + word] = after;
                    changed |= before != after;
                }
                if changed {
                    charge(work)?;
                    pending.push(*to);
                }
            }
        }
        Ok(Self { by_point, words })
    }

    pub(in crate::loan_plan) fn uncached(
        cfg: &Cfg<'_>,
        starts: impl ExactSizeIterator<Item = u16> + DoubleEndedIterator,
        work: &mut WorkCounter,
    ) -> Result<Self, Diagnostic> {
        let words = starts.len().div_ceil(u64::BITS as usize);
        let mut by_point = vec![0; words * cfg.points.len()].into_boxed_slice();
        for (loan, start) in starts.enumerate() {
            let mut reached = vec![false; cfg.points.len()];
            let mut pending = vec![start];
            while let Some(node) = pending.pop() {
                charge(work)?;
                if !reached[node as usize] {
                    reached[node as usize] = true;
                    by_point[node as usize * words + loan / 64] |= 1u64 << (loan % 64);
                    pending.extend(cfg.successors[node as usize].iter().rev().copied());
                }
            }
        }
        Ok(Self { by_point, words })
    }

    pub(in crate::loan_plan) fn contains(&self, loan: LoanId, node: u16) -> bool {
        let loan = loan.0 as usize;
        self.by_point[node as usize * self.words + loan / 64] & (1u64 << (loan % 64)) != 0
    }
}

pub(in crate::loan_plan) fn live_nodes(
    cfg: &Cfg<'_>,
    loan: LoanId,
    start: u16,
    seeds: &BTreeSet<u16>,
    reachable: &Reachability,
    work: &mut WorkCounter,
) -> Result<BTreeSet<u16>, Diagnostic> {
    let mut live = BTreeSet::new();
    let mut pending = Vec::new();
    for seed in seeds {
        charge(work)?;
        if reachable.contains(loan, *seed) {
            pending.push(*seed);
        }
    }
    pending.push(start);
    while let Some(node) = pending.pop() {
        charge(work)?;
        if !reachable.contains(loan, node) || !live.insert(node) || node == start {
            continue;
        }
        pending.extend(cfg.predecessors[node as usize].iter().rev().copied());
    }
    Ok(live)
}

pub(in crate::loan_plan) fn edge_liveness(
    cfg: &Cfg<'_>,
    live: &[BTreeSet<u16>],
    work: &mut WorkCounter,
) -> Result<EdgeLiveness, Diagnostic> {
    let mut edge_live = vec![Vec::<LoanId>::new(); cfg.edges.len()];
    let mut termination_edges = vec![Vec::<u16>::new(); live.len()];
    for (loan_index, nodes) in live.iter().enumerate() {
        let id = LoanId(loan_index as u16);
        for from in nodes {
            for edge_index in &cfg.successor_edges[*from as usize] {
                charge(work)?;
                let (_, to) = cfg.edges[*edge_index as usize];
                if nodes.contains(&to) {
                    edge_live[*edge_index as usize].push(id);
                } else {
                    termination_edges[loan_index].push(*edge_index);
                }
            }
        }
    }
    Ok((edge_live, termination_edges))
}
