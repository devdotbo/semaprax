//! Opt-in observations, never a deadline or acceptance authority.
use std::time::{Duration, Instant};

pub(super) struct Timing {
    origin: Instant,
    pub(super) spawn_return: Option<Duration>,
    pub(super) deadline_start: Option<Duration>,
    pub(super) first_wait: Option<Duration>,
    pub(super) terminal: Option<Duration>,
    pub(super) terminal_kind: Option<&'static str>,
    pub(super) stdout_settled: Option<Duration>,
    pub(super) stderr_settled: Option<Duration>,
    pub(super) complete: Option<Duration>,
}
impl Timing {
    pub(super) fn new() -> Self {
        Self {
            origin: Instant::now(),
            spawn_return: None,
            deadline_start: None,
            first_wait: None,
            terminal: None,
            terminal_kind: None,
            stdout_settled: None,
            stderr_settled: None,
            complete: None,
        }
    }
    pub(super) fn elapsed(&self) -> Duration {
        self.origin.elapsed()
    }
    pub(super) fn deadline_started(&mut self, started: Instant) {
        self.deadline_start = Some(started.saturating_duration_since(self.origin));
    }
    pub(super) fn terminal(&mut self, kind: &'static str) {
        self.terminal = Some(self.elapsed());
        self.terminal_kind = Some(kind);
    }
    pub(super) fn json(&self, timeout_ms: u64) -> serde_json::Value {
        let seconds = |value: Option<Duration>| value.map(|value| value.as_secs_f64());
        serde_json::json!({
            "schema": "semaprax.native-test-diagnostic-timing.v1",
            "scope": "monotonic observations relative to run_case entry; compilation and scratch sealing/cleanup excluded",
            "timeout_ms": timeout_ms,
            "spawn_return_seconds": seconds(self.spawn_return),
            "deadline_start_seconds": seconds(self.deadline_start),
            "first_wait_seconds": seconds(self.first_wait),
            "terminal_observed_seconds": seconds(self.terminal),
            "terminal_kind": self.terminal_kind,
            "stdout_settled_seconds": seconds(self.stdout_settled),
            "stderr_settled_seconds": seconds(self.stderr_settled),
            "observation_complete_seconds": seconds(self.complete),
            "deadline_interval_seconds": seconds(self.terminal.zip(self.deadline_start).map(|(end,start)| end.saturating_sub(start))),
            "note": "process loader and scheduling remain in deadline interval; timestamps do not establish their individual costs"
        })
    }
}
