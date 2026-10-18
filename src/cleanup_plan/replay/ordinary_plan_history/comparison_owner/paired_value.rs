//! Bare pair-local reference yields; exhaustion consumes its lexical branch.
use super::*;
// One additional right expansion; all five real primitive frontiers are paid
// even for zero pairs. No frozen old invoice is credited or changed.
pub(super) const ADDITIONAL_PRIMITIVES: usize = 5 * PAYMENT_HEADER;
pub(super) const STEP: usize = 2 * size_of::<usize>() + size_of::<bool>() // retained segment extent and condition
    + 2 * size_of::<bool>() // loaded condition and negation
    + size_of::<&Rc<SkeletonObservation>>() // head pattern
    + 2 * size_of::<&[Rc<SkeletonObservation>]>() // rest pattern and assignment copy
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>() // as_ref actual/formal/return
    + size_of::<usize>(); // original item increment
// Only entering a new immutable segment forms these views/index receivers.
pub(super) const LOAD: usize = 2 * size_of::<&mut Vec<usize>>()
    + size_of::<Option<usize>>()
    + size_of::<usize>()
    + size_of::<bool>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<bool>(); // loaded=true
// Pair-local borrowed suffix: ref-rest pattern plus assignment, copied link,
// failed-pattern branch and unchanged arena/index/view/loaded receivers.
pub(super) const PAIR_LOAD: usize = 2 * size_of::<&[usize]>()
    + size_of::<usize>()
    + size_of::<bool>()
    + 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<bool>();
const _: () = assert!(PAIR_LOAD == LOAD);
// The original exhausted-segment update and fresh tail remain honestly paid.
pub(super) const EMPTY: usize = 2 * size_of::<usize>() + size_of::<bool>(); // segment increment, item=0, loaded=false
pub(super) const LINK_VIEW: usize = 2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>();
pub(super) const TAIL_VIEW: usize =
    2 * size_of::<&Vec<Rc<SkeletonObservation>>>() + 2 * size_of::<&[Rc<SkeletonObservation>]>();
pub(super) const TAIL: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>()
    + size_of::<usize>();

macro_rules! lexical_plan_value {
    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident, $link:ident, [$($head_value:ident => $entry:block)?],
     $tail:ident, $links:ident, $function:ident, $budget:ident, $phase:literal, $census:ident, $empty:block) => {{
        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
        'plan_yield: {
            loop {
                paid!(
                    $budget,
                    $function,
                    paired_value::STEP + paired_value::EMPTY,
                    $phase
                )?;
                #[cfg(test)]
                $census.value_step();
                #[cfg(test)]
                $census.step();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::StepAdmitted,
                    paired_value::STEP + paired_value::EMPTY,
                );
                if $owner.$cursor.segment >= $owner.$limit {
                    break;
                }
                $budget.charge(
                    $function,
                    1,
                    "ordinary plan history forward segment traversal",
                )?;
                if !$owner.$loaded {
                    paid!(
                        $budget,
                        $function,
                        paired_value::PAIR_LOAD,
                        $phase
                    )?;
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::LoadAdmitted,
                        paired_value::PAIR_LOAD,
                    );
                    #[cfg(test)]
                    $census.load();
                    let &[ref links_rest @ .., $link] = $links else {
                        unreachable!("prepared ordinary plan links");
                    };
                    $links = links_rest;
                    $owner.$items = $owner.arena[$link].items.as_slice();
                    $owner.$loaded = true;
                    $(
                        // First-head provenance and the already-paid pop's link
                        // remain lexical. The continuation always leaves this
                        // successful head; [] still takes the original path.
                        match $owner.$items {
                            [head, rest @ ..] => {
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    if stringify!($cursor) == "left_cursor" {
                                        $crate::workflow_profile::plan_comparison::Site::LeftHead
                                    } else {
                                        $crate::workflow_profile::plan_comparison::Site::RightHead
                                    }, 0);
                                $owner.$items = rest;
                                $owner.$cursor.item += 1;
                                let $head_value = head.as_ref();
                                $entry
                            }
                            [] => {}
                        }
                    )?
                }
                match $owner.$items {
                    [head, rest @ ..] => {
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            if stringify!($cursor) == "left_cursor" {
                                $crate::workflow_profile::plan_comparison::Site::LeftHead
                            } else {
                                $crate::workflow_profile::plan_comparison::Site::RightHead
                            },
                            0,
                        );
                        $owner.$items = rest;
                        $owner.$cursor.item += 1;
                        break 'plan_yield head.as_ref();
                    }
                    [] => {
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::EmptySegment,
                            0,
                        );
                        $owner.$cursor.segment += 1;
                        $owner.$cursor.item = 0;
                        $owner.$loaded = false;
                    }
                }
            }
            paid!($budget, $function, paired_value::TAIL + paired_value::TAIL_MARK, $phase)?;
            mark_plan_tail!($owner, $cursor);
            #[cfg(test)] $census.tail_marks();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailMarkAdmitted, paired_value::TAIL_MARK);
            #[cfg(test)]
            $census.value_tail();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailAdmitted,
                paired_value::TAIL,
            );
            match $tail {
                [head, rest @ ..] => {
                    $tail = rest;
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        if stringify!($cursor) == "left_cursor" {
                            $crate::workflow_profile::plan_comparison::Site::LeftTailSome
                        } else {
                            $crate::workflow_profile::plan_comparison::Site::RightTailSome
                        },
                        0,
                    );
                    $owner.$cursor.item += 1;
                    head.as_ref()
                }
                [] => $empty,
            }
        }
    }};
}

// Five actual new primitive sites: selection reserve, bulk reserve and three
// original work calls. No old bank is reduced. The field itself is in sizeof
// ComparisonOwner; this bank additionally owns its literal initializer.
pub(super) const TAIL_OWNER: usize = 5 * PAYMENT_HEADER + size_of::<u8>();
pub(super) const TAIL_SELECT: usize = 2 * size_of::<u8>() + size_of::<bool>();
pub(super) const TAIL_MARK: usize = 3 * size_of::<u8>();
pub(super) const TAIL_TURN: usize = 2 * size_of::<&Rc<SkeletonObservation>>()
    + 4 * size_of::<&[Rc<SkeletonObservation>]>()
    + 4 * size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonObservation>()
    + 4 * size_of::<&SkeletonTerminal>()
    + 2 * size_of::<usize>()
    + 4 * size_of::<Ordering>()
    + 3 * size_of::<bool>();
macro_rules! mark_plan_tail {
    ($owner:ident, left_cursor) => {
        $owner.tail_mask |= 1;
    };
    ($owner:ident, right_cursor) => {
        $owner.tail_mask |= 2;
    };
}
macro_rules! fused_plan_tails {
    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident,
     $function:ident, $budget:ident, $census:ident, $paired:lifetime) => {{
        paid!($budget, $function, paired_value::TAIL_SELECT, "ordinary plan paired tail selection")?;
        #[cfg(test)] $census.mask_checks();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::TailMaskAdmitted, paired_value::TAIL_SELECT);
        if $owner.tail_mask == 3 {
            loop {
                paid!($budget, $function, paired_value::TAIL_TURN, "ordinary plan paired tail turn")?;
                #[cfg(test)] $census.fused_turns();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::TailTurnAdmitted, paired_value::TAIL_TURN);
                $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                #[cfg(test)] $census.tail_work();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::TailWork, 0);
                #[cfg(test)] pair_scope_census::maybe_unwind_tail_work();
                $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                #[cfg(test)] $census.tail_work();
                #[cfg(feature = "unstable-workflow-profiling")]
                $crate::workflow_profile::plan_comparison::event(
                    $crate::workflow_profile::plan_comparison::Site::TailWork, 0);
                #[cfg(test)] pair_scope_census::maybe_unwind_tail_work();
                match $left_tail {
                    [left, left_rest @ ..] => {
                        $left_tail = left_rest;
                        $owner.left_cursor.item += 1;
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::LeftTailSome, 0);
                        match $right_tail {
                            [right, right_rest @ ..] => {
                                $right_tail = right_rest;
                                $owner.right_cursor.item += 1;
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightTailSome, 0);
                                $owner.ordering = left.as_ref().cmp(right.as_ref());
                                if $owner.ordering == Ordering::Equal {
                                    $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                                    #[cfg(test)] { $census.tail_work(); $census.round(); }
                                    #[cfg(feature = "unstable-workflow-profiling")]
                                    { $crate::workflow_profile::plan_comparison::event(
                                        $crate::workflow_profile::plan_comparison::Site::TailWork, 0);
                                      $crate::workflow_profile::plan_comparison::event(
                                        $crate::workflow_profile::plan_comparison::Site::Round, 0); }
                                    #[cfg(test)] pair_scope_census::maybe_unwind_tail_work();
                                    continue;
                                }
                            }
                            [] => {
                                $owner.ordering = Ordering::Greater;
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightExhausted, 0);
                            }
                        }
                    }
                    [] => {
                        #[cfg(feature = "unstable-workflow-profiling")]
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::LeftExhausted, 0);
                        match $right_tail {
                            [_, right_rest @ ..] => {
                                $right_tail = right_rest;
                                $owner.right_cursor.item += 1;
                                $owner.ordering = Ordering::Less;
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightTailSome, 0);
                            }
                            [] => {
                                $owner.ordering = $left_path.terminal.cmp(&$right_path.terminal);
                                #[cfg(feature = "unstable-workflow-profiling")]
                                { $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::RightExhausted, 0);
                                  $crate::workflow_profile::plan_comparison::event(
                                    $crate::workflow_profile::plan_comparison::Site::TerminalCompared, 0); }
                            }
                        }
                    }
                }
                break $paired;
            }
        }
    }};
}
