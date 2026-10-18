//! Two lexical first heads certify an immutable segment, never a saved verdict.
use super::{Ordering, PAYMENT_HEADER, Rc, Segment, SkeletonObservation, size_of};
// Additional actual reserve/work sites relative to the complete d458 owner:
// split right producer +3, relocated MATCH +3, first-head certificate +1.
pub(super) const OWNER: usize = 7 * PAYMENT_HEADER;
pub(super) const BASE: usize =
    2 * size_of::<usize>() + 3 * size_of::<bool>() + super::paired_value::EMPTY;
pub(super) const POP: usize = 2 * size_of::<&mut Vec<usize>>()
    + size_of::<Option<usize>>()
    + size_of::<usize>()
    + 2 * size_of::<bool>();
pub(super) const VIEW_HEAD: usize = 2 * size_of::<&[Segment]>()
    + 2 * size_of::<usize>()
    + size_of::<&Segment>()
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + HEAD;
pub(super) const HEAD: usize = size_of::<&Rc<SkeletonObservation>>()
    + 2 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>()
    + size_of::<usize>();
// The old as_ref invoice remains conservative admission on the identity branch;
// it is not a claim that an observation receiver is physically produced there.
pub(super) const RHS: usize = size_of::<&[Rc<SkeletonObservation>]>()
    + size_of::<usize>()
    + 2 * size_of::<&Rc<SkeletonObservation>>()
    + size_of::<&SkeletonObservation>();
pub(super) const FIRST: usize = 2 * size_of::<&SkeletonObservation>() + size_of::<Ordering>();

macro_rules! certified_match {
    ($budget:ident, $function:ident, $census:ident) => {{
        paid!(
            $budget,
            $function,
            PAIR_MATCH,
            "ordinary plan history paired cursor next"
        )?;
        #[cfg(test)]
        $census.pair_match_admitted();
        #[cfg(feature = "unstable-workflow-profiling")]
        $crate::workflow_profile::plan_comparison::event(
            $crate::workflow_profile::plan_comparison::Site::MatchAdmitted,
            PAIR_MATCH,
        );
    }};
}
macro_rules! certified_right_value {
    ($owner:ident, $left_head:ident, $left_link:ident, $tail:ident,
     $function:ident, $budget:ident, $census:ident, $paired:lifetime) => {{
        // Original observation receiving slots, formed only on fallback. There
        // is no new Option, tuple, callback, result or Rc alias transport.
        let left;
        let right;
        'right_observation: {
            $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
            loop {
                paid!($budget, $function, certified_heads::BASE,
                    "ordinary plan fresh right segment base")?;
                #[cfg(test)] $census.certified_base();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedBaseAdmitted,
                        certified_heads::BASE);
                if $owner.right_cursor.segment >= $owner.right_segments { break; }
                $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
                if !$owner.right_loaded {
                    paid!($budget, $function, certified_heads::POP,
                        "ordinary plan fresh right link pop")?;
                    #[cfg(test)] $census.certified_pop();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedPopAdmitted,
                        certified_heads::POP);
                    let Some(right_link) = $owner.right_cursor.links.pop() else {
                        unreachable!("prepared ordinary plan links");
                    };
                    paid!($budget, $function, shared_prefix::DETECTOR,
                        "ordinary plan fresh segment identity event")?;
                    #[cfg(test)] $census.identity_detector();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityDetectorAdmitted,
                        shared_prefix::DETECTOR);
                    if $left_link == right_link {
                        paid!($budget, $function, certified_heads::RHS,
                            "ordinary plan certified right slice assignment")?;
                        #[cfg(test)] $census.certified_rhs();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedRhsAdmitted,
                        certified_heads::RHS);
                        $owner.right_items = $owner.left_items;
                        $owner.right_loaded = true;
                        $owner.right_cursor.item += 1;
                        paid!($budget, $function, certified_heads::FIRST,
                            "ordinary plan certified first observation")?;
                        #[cfg(test)] $census.certified_first();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedFirstAdmitted,
                        certified_heads::FIRST);
                        $owner.ordering = Ordering::Equal;
                        // This is the original comparison work, after both
                        // cursor advances and before any suffix probe.
                        $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                        #[cfg(test)] $census.round();
                        #[cfg(test)] $census.identity_hit();
                        #[cfg(feature = "unstable-workflow-profiling")]
                        {
                            $crate::workflow_profile::plan_comparison::event(
                                $crate::workflow_profile::plan_comparison::Site::Round, 0);
                            $crate::workflow_profile::plan_comparison::event(
                                $crate::workflow_profile::plan_comparison::Site::IdentityHit, 0);
                        }
                        certified_suffix!($owner, $function, $budget, $census, $paired);
                    }
                    paid!($budget, $function, certified_heads::VIEW_HEAD,
                        "ordinary plan foreign right segment view and head")?;
                    #[cfg(test)] $census.certified_view();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedViewAdmitted,
                        certified_heads::VIEW_HEAD);
                    $owner.right_items = $owner.arena[right_link].items.as_slice();
                    $owner.right_loaded = true;
                } else {
                    paid!($budget, $function, certified_heads::HEAD,
                        "ordinary plan loaded right head")?;
                    #[cfg(test)] $census.certified_head();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::CertifiedHeadAdmitted,
                        certified_heads::HEAD);
                }
                match $owner.right_items {
                    [head, rest @ ..] => {
                        $owner.right_items = rest;
                        $owner.right_cursor.item += 1;
                        certified_match!($budget, $function, $census);
                        left = $left_head.as_ref();
                        right = head.as_ref();
                        break 'right_observation;
                    }
                    [] => {
                        $owner.right_cursor.segment += 1;
                        $owner.right_cursor.item = 0;
                        $owner.right_loaded = false;
                    }
                }
            }
            paid!($budget, $function, paired_value::TAIL,
                "ordinary plan history paired cursor next")?;
            #[cfg(test)] $census.value_tail();
            certified_match!($budget, $function, $census);
            match $tail {
                [head, rest @ ..] => {
                    $tail = rest;
                    $owner.right_cursor.item += 1;
                    left = $left_head.as_ref();
                    right = head.as_ref();
                }
                [] => { $owner.ordering = Ordering::Greater; break $paired; }
            }
        }
        shared_prefix_first_comparison!($owner, left, right, $function, $budget,
            $census, $paired, { continue $paired; });
    }};
}

// BEGIN LEFT FIRST-HEAD PRODUCER
// Left first-head continuation retains the original Rc pattern loan.
macro_rules! certified_left_value {
    ($owner:ident, $cursor:ident, $items:ident, $loaded:ident, $limit:ident, $link:ident, [$($head_value:ident => $entry:block)?],
     $tail:ident, $function:ident, $budget:ident, $phase:literal, $census:ident, $empty:block) => {{
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
                        paired_value::LOAD,
                        $phase
                    )?;
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::LoadAdmitted,
                        paired_value::LOAD,
                    );
                    #[cfg(test)]
                    $census.load();
                    let Some($link) = $owner.$cursor.links.pop() else {
                        unreachable!("prepared ordinary plan links");
                    };
                    $owner.$items = $owner.arena[$link].items.as_slice();
                    $owner.$loaded = true;
                    $(
                        // First-head provenance and the already-paid pop's link
                        // remain lexical. The continuation always leaves this
                        // successful head; [] still takes the original path.
                        match $owner.$items {
                            [$head_value, rest @ ..] => {
                                #[cfg(feature = "unstable-workflow-profiling")]
                                $crate::workflow_profile::plan_comparison::event(
                                    if stringify!($cursor) == "left_cursor" {
                                        $crate::workflow_profile::plan_comparison::Site::LeftHead
                                    } else {
                                        $crate::workflow_profile::plan_comparison::Site::RightHead
                                    }, 0);
                                $owner.$items = rest;
                                $owner.$cursor.item += 1;
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
                        certified_match!($budget, $function, $census);
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
            paid!($budget, $function, paired_value::TAIL, $phase)?;
            #[cfg(test)]
            $census.value_tail();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::TailAdmitted,
                paired_value::TAIL,
            );
            certified_match!($budget, $function, $census);
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

// BEGIN CERTIFIED SUFFIX
// No second detector: the enclosing fresh-ID branch is the certificate.
macro_rules! certified_suffix {
    ($owner:ident, $function:ident, $budget:ident, $census:ident, $paired:lifetime) => {{
        loop {
            paid!($budget, $function, shared_prefix::PROBE,
                "ordinary plan shared suffix pattern probe")?;
            #[cfg(test)]
            $census.identity_probe();
            #[cfg(feature = "unstable-workflow-profiling")]
            $crate::workflow_profile::plan_comparison::event(
                $crate::workflow_profile::plan_comparison::Site::IdentityProbeAdmitted,
                shared_prefix::PROBE);
            match $owner.left_items {
                [_, rest @ ..] => {
                    paid!($budget, $function, shared_prefix::REPLAY,
                        "ordinary plan shared suffix scalar receivers")?;
                    #[cfg(test)] $census.identity_body();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityReplayAdmitted,
                        shared_prefix::REPLAY);
                    $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                    #[cfg(test)] $census.identity_work();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                    $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
                    #[cfg(test)] $census.identity_work();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                    $owner.left_items = rest;
                    $owner.left_cursor.item += 1;
                    $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
                    #[cfg(test)] $census.identity_work();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                    $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
                    #[cfg(test)] $census.identity_work();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                    $owner.right_items = rest;
                    $owner.right_cursor.item += 1;
                    $owner.ordering = Ordering::Equal;
                    #[cfg(test)] $census.identity_observation();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityObservation, 0);
                    $budget.charge($function, 1, "ordinary plan history observation comparison")?;
                    #[cfg(test)] $census.identity_work();
                    #[cfg(test)] $census.round();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    {
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::IdentityWork, 0);
                        $crate::workflow_profile::plan_comparison::event(
                            $crate::workflow_profile::plan_comparison::Site::Round, 0);
                    }
                }
                [] => {
                    #[cfg(test)] $census.identity_empty_probe();
                    #[cfg(feature = "unstable-workflow-profiling")]
                    $crate::workflow_profile::plan_comparison::event(
                        $crate::workflow_profile::plan_comparison::Site::IdentityEmptyProbe, 0);
                    break;
                }
            }
        }
        continue $paired;
    }};
}
