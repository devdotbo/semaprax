//! Lexical forward cursor. No next callee or compound fallible return exists.
use super::*;

// Narrow self-bootstrap bank for the two new reserve primitive sites at each
// of four lexical expansions. This uses only the original comparator's
// sequential primitive-slot contract: ZST success, one terminal Diagnostic,
// no escaping/overlapping activation, stable function/budget places. It does
// not cover work results or any get/Option/borrow/payload below. The distinct
// forward work call is paid EVERY next; segment work EVERY segment test.
// Whether that inherited primitive contract admits these newly introduced
// payment sites remains an explicit independent-review obligation.
pub(super) const PRIMITIVE_FRAMES: usize = 8 * PAYMENT_HEADER;
// Per next execution, before any lexical views are formed. These are actual
// views/receivers, not the removed callee's formal references or Result.
pub(super) const ENTRY: usize = PAYMENT_HEADER // actual forward charge arguments/formals + return/receive
    + size_of::<&mut Cursor>() // direct named owner field borrow; no index or formal
    + size_of::<&History>() // borrowed history field, no forwarding formal
    + 2 * size_of::<&Vec<Segment>>() + 2 * size_of::<&[Segment]>() // as_slice actual/formal, return/local
    + 2 * size_of::<&Vec<usize>>() + 2 * size_of::<&[usize]>(); // links as_slice and return/local
// Acquired on EVERY loop test, including the exhausted sentinel. No retained
// Option or static bank exempts a recreated get/index/match payload. The
// segment arm's possible slots are acquired before the condition; its original
// segment work remains before every lookup. A false test honestly pays this
// complete envelope and then constructs none of that arm's payloads.
pub(super) const SEGMENT: usize = PAYMENT_HEADER // actual segment charge, including both Result slots
    + 4 * size_of::<&[usize]>() + 2 * size_of::<usize>() // two len calls/returns
    + size_of::<bool>() // loop test
    + 2 * size_of::<usize>() // left-to-right subtraction intermediates
    + 2 * size_of::<&[usize]>() + 2 * size_of::<usize>() // link index actual/formal
    + size_of::<&usize>() + size_of::<usize>() // index return and copied link
    + 2 * size_of::<&[Segment]>() + 2 * size_of::<usize>() // segment index
    + size_of::<&Segment>() + size_of::<&Vec<Rc<SkeletonObservation>>>() // return/field local
    + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>() // Vec deref actual/formal
    + 3 * size_of::<&[Rc<SkeletonObservation>]>() // deref return, get actual/formal
    + 2 * size_of::<usize>() // get index actual/formal
    + 2 * size_of::<Option<&Rc<SkeletonObservation>>>() // get return and match receiver
    + size_of::<&Rc<SkeletonObservation>>() // successful pattern binding
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>() // as_ref
    + 2 * size_of::<Option<&SkeletonObservation>>() // yielded Some and caller binding
    + 2 * size_of::<usize>(); // item increment / segment increment
pub(super) const TAIL: usize =
    2 * size_of::<&Vec<Rc<SkeletonObservation>>>()
    + 3 * size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * size_of::<usize>()
    + 3 * size_of::<Option<&Rc<SkeletonObservation>>>() // get return/local and consuming match
    + 2 * size_of::<&Option<&Rc<SkeletonObservation>>>() + size_of::<bool>() // is_some
    + size_of::<usize>() // item increment
    + size_of::<&Rc<SkeletonObservation>>() // successful tail pattern
    + 2 * size_of::<&Rc<SkeletonObservation>>() + size_of::<&SkeletonObservation>()
    + 2 * size_of::<Option<&SkeletonObservation>>();

macro_rules! lexical_plan_next {
    ($owner:ident, $field:ident, $history:expr, $arena:expr,
     $function:ident, $budget:ident, $phase:literal) => {{
        $budget.charge($function, 1, "ordinary plan history forward cursor step")?;
        let cursor = &mut $owner.$field;
        let history = &$history;
        let arena: &[Segment] = $arena.as_slice();
        let links = cursor.links.as_slice();
        'plan_yield: {
            loop {
                paid!($budget, $function, lexical_cursor::SEGMENT, $phase)?;
                if cursor.segment >= links.len() { break; }
                $budget.charge($function, 1, "ordinary plan history forward segment traversal")?;
                let link = links[links.len() - cursor.segment - 1];
                let items = &arena[link].items;
                if let Some(value) = items.get(cursor.item) {
                    cursor.item += 1;
                    break 'plan_yield Some::<&SkeletonObservation>(value.as_ref());
                }
                cursor.segment += 1;
                cursor.item = 0;
            }
            paid!($budget, $function, lexical_cursor::TAIL, $phase)?;
            let value = history.tail.get(cursor.item);
            if value.is_some() { cursor.item += 1; }
            match value {
                Some(value) => Some::<&SkeletonObservation>(value.as_ref()),
                None => None,
            }
        }
    }};
}
