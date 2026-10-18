//! Diagnostic-only, current-thread observation of the ordinary PLAN comparator.
//! Fixed TLS storage is outside the compiler's allocation ledger. No timings,
//! allocations, admission decisions, or default-shipping performance are measured.
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

pub const ROW_CAPACITY: usize = 128;
const DEPTH: usize = 32;
/// A prefix is never presented as the full identifier when truncated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextIdentity {
    pub prefix: [u8; 96],
    pub prefix_len: usize,
    pub full_len: usize,
    pub sha256: [u8; 32],
}
impl TextIdentity {
    const EMPTY: Self = Self {
        prefix: [0; 96],
        prefix_len: 0,
        full_len: 0,
        sha256: [0; 32],
    };
    fn new(text: &str) -> Self {
        let mut value = Self::EMPTY;
        value.full_len = text.len();
        value.prefix_len = text.len().min(value.prefix.len());
        value.prefix[..value.prefix_len].copy_from_slice(&text.as_bytes()[..value.prefix_len]);
        value.sha256 = Sha256::digest(text.as_bytes()).into();
        value
    }
}
/// Caller-supplied fixture metadata, never source or validation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Binding {
    pub fixture: TextIdentity,
    pub source_sha256: [u8; 32],
    pub caller_revision: Option<[u8; 20]>,
}
impl Binding {
    pub fn caller_labelled(fixture: &str, source_sha256: [u8; 32]) -> Self {
        Self {
            fixture: TextIdentity::new(fixture),
            source_sha256,
            caller_revision: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Payer {
    Unobserved,
    ActiveBuilder,
    SelectedReservation,
    Oracle,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Build {
    InstrumentedProductionAlgorithm,
    InstrumentedLibraryTest,
}
/// Closed sites. Counts describe reached prefixes, not successful admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum Site {
    OwnerAdmitted,
    PairPrepared,
    Round,
    PairScopeAdmitted,
    MatchAdmitted,
    StepAdmitted,
    LoadAdmitted,
    EmptySegment,
    TailAdmitted,
    LeftHead,
    RightHead,
    LeftTailSome,
    RightTailSome,
    LeftExhausted,
    RightExhausted,
    TerminalCompared,
    LeftPrepareAdmitted,
    RightPrepareAdmitted,
    LeftResetCompleted,
    RightResetCompleted,
}
const SITES: usize = 20;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Counts {
    pub events: [u64; SITES],
    /// Sum of actually admitted bank bytes at the corresponding closed site.
    pub admitted_bytes: [u64; SITES],
    pub started: u64,
    pub ended: u64,
    pub succeeded: u64,
    pub unwound: u64,
}
impl Counts {
    const EMPTY: Self = Self {
        events: [0; SITES],
        admitted_bytes: [0; SITES],
        started: 0,
        ended: 0,
        succeeded: 0,
        unwound: 0,
    };
    pub fn at(&self, site: Site) -> u64 {
        self.events[site as usize]
    }
}
/// Actual compiled tariffs; not a forecast of reached events or a fee credit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Invoice {
    pub payment_header: usize,
    pub owner: usize,
    pub pair_primitives: usize,
    pub paired_primitives: usize,
    pub path_index: usize,
    pub pair_scope: usize,
    pub pair_match: usize,
    pub new_pair_references: usize,
    pub step: usize,
    pub load: usize,
    pub empty: usize,
    pub tail: usize,
}
impl Invoice {
    const EMPTY: Self = Self {
        payment_header: 0,
        owner: 0,
        pair_primitives: 0,
        paired_primitives: 0,
        path_index: 0,
        pair_scope: 0,
        pair_match: 0,
        new_pair_references: 0,
        step: 0,
        load: 0,
        empty: 0,
        tail: 0,
    };
}
#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub serial: u64,
    pub function: TextIdentity,
    pub span: crate::ast::Span,
    pub payer: Payer,
    pub selected: bool,
    pub invoice: Invoice,
    pub counts: Counts,
}
impl Row {
    const EMPTY: Self = Self {
        serial: 0,
        function: TextIdentity::EMPTY,
        span: crate::ast::Span {
            start: 0,
            end: 0,
            line: 0,
            column: 0,
        },
        payer: Payer::Unobserved,
        selected: false,
        invoice: Invoice::EMPTY,
        counts: Counts::EMPTY,
    };
}
#[derive(Debug)]
pub struct Report {
    pub build: Build,
    pub route: &'static str,
    pub generation: u64,
    pub binding: Binding,
    pub counts: Counts,
    pub rows: [Row; ROW_CAPACITY],
    pub row_count: usize,
    pub detail_complete: bool,
    pub counts_complete: bool,
    pub overflow: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureError {
    Busy,
    GenerationExhausted,
    LiveInvocation,
    LostCapture,
}
struct State {
    enabled: bool,
    generation: u64,
    binding: Binding,
    rows: [Row; ROW_CAPACITY],
    row_count: usize,
    counts: Counts,
    stack: [Option<usize>; DEPTH],
    depth: usize,
    detail_complete: bool,
    complete: bool,
    overflow: bool,
}
impl State {
    const fn new() -> Self {
        Self {
            enabled: false,
            generation: 0,
            binding: Binding {
                fixture: TextIdentity::EMPTY,
                source_sha256: [0; 32],
                caller_revision: None,
            },
            rows: [Row::EMPTY; ROW_CAPACITY],
            row_count: 0,
            counts: Counts::EMPTY,
            stack: [None; DEPTH],
            depth: 0,
            detail_complete: true,
            complete: true,
            overflow: false,
        }
    }
    fn add(target: &mut u64, amount: u64, overflow: &mut bool) {
        if let Some(value) = target.checked_add(amount) {
            *target = value;
        } else {
            *overflow = true;
        }
    }
    fn each(&mut self, mut f: impl FnMut(&mut Counts, &mut bool)) {
        f(&mut self.counts, &mut self.overflow);
        if self.depth > 0 && self.depth <= DEPTH {
            if let Some(index) = self.stack[self.depth - 1] {
                f(&mut self.rows[index].counts, &mut self.overflow);
            }
        }
    }
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State::new()) };
    static LOST: Cell<bool> = const { Cell::new(false) };
}
fn access<T>(f: impl FnOnce(&mut State) -> T) -> Option<T> {
    STATE.with(|state| match state.try_borrow_mut() {
        Ok(mut state) => Some(f(&mut state)),
        Err(_) => {
            LOST.with(|lost| lost.set(true));
            None
        }
    })
}
/// Begin/finish never execute compiler work or change its result. Thread-bound.
pub struct Capture {
    generation: u64,
    _thread: PhantomData<Rc<()>>,
}
impl Capture {
    pub fn begin(binding: Binding) -> Result<Self, CaptureError> {
        access(|s| {
            if s.enabled {
                return Err(CaptureError::Busy);
            }
            let generation = s
                .generation
                .checked_add(1)
                .ok_or(CaptureError::GenerationExhausted)?;
            *s = State::new();
            s.generation = generation;
            s.binding = binding;
            s.enabled = true;
            LOST.with(|lost| lost.set(false));
            Ok(Self {
                generation,
                _thread: PhantomData,
            })
        })
        .unwrap_or(Err(CaptureError::Busy))
    }
    /// Call after compiler owners have dropped. A live invocation refuses finish.
    pub fn finish(self) -> Result<Report, CaptureError> {
        access(|s| {
            if !s.enabled || s.generation != self.generation {
                return Err(CaptureError::LostCapture);
            }
            if s.depth != 0 {
                return Err(CaptureError::LiveInvocation);
            }
            s.enabled = false;
            Ok(Report {
                build: if cfg!(test) {
                    Build::InstrumentedLibraryTest
                } else {
                    Build::InstrumentedProductionAlgorithm
                },
                route: "ordinary-plan-comparison/paired-value.v1",
                generation: s.generation,
                binding: s.binding,
                counts: s.counts,
                rows: s.rows,
                row_count: s.row_count,
                detail_complete: s.detail_complete,
                counts_complete: s.complete && !s.overflow && !LOST.with(Cell::get),
                overflow: s.overflow,
            })
        })
        .unwrap_or(Err(CaptureError::LostCapture))
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        access(|s| {
            if s.generation == self.generation {
                s.enabled = false;
            }
        });
    }
}
/// Private compiler guard: stores no references into a function, arena or payer.
pub(crate) struct Invocation {
    generation: Option<u64>,
    depth: usize,
    succeeded: bool,
}
pub(crate) fn begin(
    function: &str,
    span: crate::ast::Span,
    payer: Payer,
    selected: bool,
    invoice: Invoice,
) -> Invocation {
    let identity = access(|s| {
        if !s.enabled {
            return None;
        }
        let serial = match s.counts.started.checked_add(1) {
            Some(serial) => serial,
            None => {
                s.overflow = true;
                0
            }
        };
        let row = if s.row_count < ROW_CAPACITY {
            let index = s.row_count;
            s.row_count += 1;
            s.rows[index] = Row {
                serial,
                function: TextIdentity::new(function),
                span,
                payer,
                selected,
                invoice,
                counts: Counts::EMPTY,
            };
            Some(index)
        } else {
            s.detail_complete = false;
            None
        };
        if s.depth < DEPTH {
            s.stack[s.depth] = row;
        } else {
            s.detail_complete = false;
        }
        match s.depth.checked_add(1) {
            Some(depth) => s.depth = depth,
            None => {
                s.complete = false;
                return None;
            }
        }
        s.each(|c, overflow| State::add(&mut c.started, 1, overflow));
        Some((s.generation, s.depth))
    })
    .flatten();
    Invocation {
        generation: identity.map(|x| x.0),
        depth: identity.map_or(0, |x| x.1),
        succeeded: false,
    }
}
pub(crate) fn event(site: Site, bytes: usize) {
    access(|s| {
        if s.enabled && s.depth != 0 {
            s.each(|c, overflow| {
                State::add(&mut c.events[site as usize], 1, overflow);
                match u64::try_from(bytes) {
                    Ok(bytes) => State::add(&mut c.admitted_bytes[site as usize], bytes, overflow),
                    Err(_) => *overflow = true,
                }
            });
        }
    });
}
impl Invocation {
    pub(crate) fn success(&mut self) {
        self.succeeded = true;
    }
}
impl Drop for Invocation {
    fn drop(&mut self) {
        access(|s| {
            if self.generation != Some(s.generation) || !s.enabled {
                return;
            }
            if s.depth != self.depth {
                s.complete = false;
                return;
            }
            s.each(|c, overflow| {
                State::add(&mut c.ended, 1, overflow);
                if self.succeeded {
                    State::add(&mut c.succeeded, 1, overflow);
                }
                if std::thread::panicking() {
                    State::add(&mut c.unwound, 1, overflow);
                }
            });
            s.depth -= 1;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding() -> Binding {
        Binding::caller_labelled("synthetic compiler observation", [17; 32])
    }
    fn invocation(id: &str) -> Invocation {
        begin(
            id,
            crate::ast::Span {
                start: 1,
                end: 9,
                line: 2,
                column: 3,
            },
            Payer::ActiveBuilder,
            true,
            Invoice::EMPTY,
        )
    }
    #[test]
    fn reached_prefix_and_success_have_separate_exact_counts() {
        let capture = Capture::begin(binding()).unwrap();
        {
            let mut guard = invocation("first");
            event(Site::PairPrepared, 0);
            event(Site::Round, 0);
            event(Site::StepAdmitted, 73);
            event(Site::LeftExhausted, 0);
            guard.success();
        }
        {
            let _refused = invocation("refused-before-owner-bank");
        }
        let report = capture.finish().unwrap();
        assert_eq!(report.build, Build::InstrumentedLibraryTest);
        assert!(report.counts_complete && report.detail_complete);
        assert_eq!(
            (
                report.counts.started,
                report.counts.ended,
                report.counts.succeeded
            ),
            (2, 2, 1)
        );
        assert_eq!(report.counts.at(Site::PairPrepared), 1);
        assert_eq!(
            report.counts.admitted_bytes[Site::StepAdmitted as usize],
            73
        );
        assert_eq!(report.rows[1].counts.at(Site::OwnerAdmitted), 0);
        let expected: [u8; 32] = Sha256::digest(b"refused-before-owner-bank").into();
        assert_eq!(report.rows[1].function.sha256, expected);
        assert_eq!(report.rows[0].span.start, 1);
    }
    #[test]
    fn busy_capture_and_unwind_preserve_the_original_operation() {
        let capture = Capture::begin(binding()).unwrap();
        assert!(matches!(Capture::begin(binding()), Err(CaptureError::Busy)));
        let original = std::panic::catch_unwind(|| {
            let _guard = invocation("unwind");
            event(Site::Round, 0);
            panic!("original payload");
        })
        .unwrap_err();
        assert_eq!(original.downcast_ref::<&str>(), Some(&"original payload"));
        let report = capture.finish().unwrap();
        assert!(report.counts_complete);
        assert_eq!(
            (
                report.counts.ended,
                report.counts.unwound,
                report.counts.succeeded
            ),
            (1, 1, 0)
        );
        let generation = report.generation;
        let next = Capture::begin(binding()).unwrap().finish().unwrap();
        assert!(next.generation > generation);
        assert_eq!(next.counts, Counts::EMPTY);
    }
    #[test]
    fn fixed_detail_limit_keeps_aggregate_counts_honest() {
        let capture = Capture::begin(binding()).unwrap();
        for _ in 0..ROW_CAPACITY + 3 {
            let mut guard = invocation("many");
            event(Site::Round, 0);
            guard.success();
        }
        let report = capture.finish().unwrap();
        assert!(!report.detail_complete);
        assert!(report.counts_complete);
        assert_eq!(report.row_count, ROW_CAPACITY);
        assert_eq!(report.counts.started, (ROW_CAPACITY + 3) as u64);
        assert_eq!(report.counts.at(Site::Round), report.counts.started);
        assert_eq!(report.counts.ended, report.counts.started);
    }
    #[test]
    fn long_identity_is_digest_bound_and_never_claimed_complete_prefix() {
        let text = "é".repeat(100);
        let capture = Capture::begin(binding()).unwrap();
        {
            let _guard = invocation(&text);
        }
        let report = capture.finish().unwrap();
        let identity = report.rows[0].function;
        assert_eq!(identity.full_len, 200);
        assert_eq!(identity.prefix_len, 96);
        assert_eq!(&identity.prefix[..], &text.as_bytes()[..96]);
        let expected: [u8; 32] = Sha256::digest(text.as_bytes()).into();
        assert_eq!(identity.sha256, expected);
        drop(text);
        assert_eq!(report.rows[0].function, identity);
    }
    #[test]
    fn overflow_reentry_and_live_finish_refuse_complete_evidence() {
        let capture = Capture::begin(binding()).unwrap();
        access(|s| {
            s.counts.events[Site::Round as usize] = u64::MAX;
        });
        {
            let _guard = invocation("overflow");
            event(Site::Round, 0);
        }
        STATE.with(|s| {
            let _held = s.borrow_mut();
            event(Site::Round, 0);
        });
        let report = capture.finish().unwrap();
        assert!(report.overflow && !report.counts_complete);
        assert_eq!(report.counts.at(Site::Round), u64::MAX);
        let capture = Capture::begin(binding()).unwrap();
        let guard = invocation("live");
        assert!(matches!(
            capture.finish(),
            Err(CaptureError::LiveInvocation)
        ));
        drop(guard);
        assert!(
            Capture::begin(binding())
                .unwrap()
                .finish()
                .unwrap()
                .counts_complete
        );
    }
    #[test]
    fn nested_rows_and_threads_do_not_cross_attribute_events() {
        let capture = Capture::begin(binding()).unwrap();
        {
            let _outer = invocation("outer");
            event(Site::LeftHead, 0);
            {
                let _inner = invocation("inner");
                event(Site::RightHead, 0);
            }
            let child = std::thread::spawn(|| {
                let capture = Capture::begin(binding()).unwrap();
                {
                    let _guard = invocation("child");
                    event(Site::TerminalCompared, 0);
                }
                capture.finish().unwrap()
            })
            .join()
            .unwrap();
            assert_eq!(child.counts.at(Site::TerminalCompared), 1);
        }
        let report = capture.finish().unwrap();
        assert_eq!(report.counts.at(Site::TerminalCompared), 0);
        assert_eq!(report.rows[0].counts.at(Site::RightHead), 0);
        assert_eq!(report.rows[1].counts.at(Site::RightHead), 1);
        assert!(report.counts_complete);
    }
}
