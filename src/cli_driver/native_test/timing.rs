//! Opt-in observations, never a deadline or acceptance authority.
use std::time::{Duration, Instant};

pub(super) struct Timing {
    origin: Instant,
    pub(super) spawn_return: Option<Duration>,
    pub(super) deadline_start: Option<Duration>,
    pub(super) first_wait: Option<Duration>,
    wait_observations: u64,
    pending_observations: u64,
    last_wait: Option<Duration>,
    last_pending: Option<Duration>,
    max_wait_gap: Duration,
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
            wait_observations: 0,
            pending_observations: 0,
            last_wait: None,
            last_pending: None,
            max_wait_gap: Duration::ZERO,
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
    pub(super) fn wait_observed(&mut self, pending: bool) {
        self.observe_wait(self.elapsed(), pending);
    }
    fn observe_wait(&mut self, now: Duration, pending: bool) {
        self.first_wait.get_or_insert(now);
        if let Some(previous) = self.last_wait {
            self.max_wait_gap = self.max_wait_gap.max(now.saturating_sub(previous));
        }
        self.last_wait = Some(now);
        self.wait_observations = self.wait_observations.saturating_add(1);
        if pending {
            self.pending_observations = self.pending_observations.saturating_add(1);
            self.last_pending = Some(now);
        }
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
            "wait_observations": self.wait_observations,
            "pending_observations": self.pending_observations,
            "last_wait_seconds": seconds(self.last_wait),
            "last_pending_seconds": seconds(self.last_pending),
            "max_wait_gap_seconds": self.max_wait_gap.as_secs_f64(),
            "deadline_overshoot_seconds": seconds(self.terminal.zip(self.deadline_start).map(|(end,start)| end.saturating_sub(start).saturating_sub(Duration::from_millis(timeout_ms)))) ,
            "terminal_observed_seconds": seconds(self.terminal),
            "terminal_kind": self.terminal_kind,
            "stdout_settled_seconds": seconds(self.stdout_settled),
            "stderr_settled_seconds": seconds(self.stderr_settled),
            "observation_complete_seconds": seconds(self.complete),
            "deadline_interval_seconds": seconds(self.terminal.zip(self.deadline_start).map(|(end,start)| end.saturating_sub(start))),
            "note": "process loader and scheduling remain in deadline interval; poll observations do not establish child CPU, loader cost or scheduler attribution"
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Timing;
    use std::time::Duration;

    #[test]
    fn polling_observations_separate_pending_checks_gaps_and_deadline_overshoot() {
        let mut timing = Timing::new();
        timing.deadline_start = Some(Duration::from_millis(1));
        timing.observe_wait(Duration::from_millis(2), true);
        timing.observe_wait(Duration::from_millis(7), true);
        timing.observe_wait(Duration::from_millis(106), true);
        timing.terminal = Some(Duration::from_millis(106));
        timing.terminal_kind = Some("deadline");
        let observed = timing.json(100);
        assert_eq!(observed["timeout_ms"], 100);
        assert_eq!(observed["wait_observations"], 3);
        assert_eq!(observed["pending_observations"], 3);
        assert_eq!(observed["max_wait_gap_seconds"], 0.099);
        assert_eq!(observed["deadline_interval_seconds"], 0.105);
        assert_eq!(observed["deadline_overshoot_seconds"], 0.005);
        assert_eq!(observed["last_pending_seconds"], 0.106);
        // A completed poll is a wait observation, not another pending child.
        timing.observe_wait(Duration::from_millis(110), false);
        let completed = timing.json(100);
        assert_eq!(completed["wait_observations"], 4);
        assert_eq!(completed["pending_observations"], 3);
        assert_eq!(completed["last_pending_seconds"], 0.106);
        assert_eq!(completed["max_wait_gap_seconds"], 0.099);
        let empty = Timing::new().json(100);
        assert_eq!(empty["wait_observations"], 0);
        assert!(empty["deadline_overshoot_seconds"].is_null());
    }
}
