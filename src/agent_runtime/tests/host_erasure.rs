//! Borrowed dispatch witnesses; no second generic execution engine.
use super::*;
use crate::test_allocation_observer::measure;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Call {
    Empty,
    Epoch(u64),
    Elapsed(u64),
    Probe,
    Tokenize([u8; 32], usize),
    Provider([u8; 32], usize, u64),
    ToolDeadline(u64),
    Tool([u8; 32], [u8; 32]),
}
struct Log {
    entries: [Call; 256],
    len: usize,
}
impl Log {
    fn new() -> Self {
        Self {
            entries: [Call::Empty; 256],
            len: 0,
        }
    }
    fn push(&mut self, value: Call) {
        assert!(self.len < self.entries.len());
        self.entries[self.len] = value;
        self.len += 1;
    }
    fn entries(&self) -> &[Call] {
        &self.entries[..self.len]
    }
}
struct Recorded<'a> {
    inner: FakeHost,
    log: &'a RefCell<Log>,
}
impl<'a> Recorded<'a> {
    fn new(log: &'a RefCell<Log>) -> Self {
        Self {
            inner: FakeHost::fixture(),
            log,
        }
    }
    fn record(&self, value: Call) {
        self.log.borrow_mut().push(value);
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
impl AgentHost for Recorded<'_> {
    fn policy_epoch(&self) -> u64 {
        let value = self.inner.policy_epoch();
        self.record(Call::Epoch(value));
        value
    }
    fn elapsed_ms(&self) -> u64 {
        let value = self.inner.elapsed_ms();
        self.record(Call::Elapsed(value));
        value
    }
    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        self.record(Call::Probe);
        // Exactly the original host's probe; no extra wrapper allocation.
        self.inner.boundary_probe()
    }
    fn tokenize(&mut self, tokenizer: &str, request: &str) -> Option<u64> {
        assert_eq!(tokenizer, "fake.bytes-v1");
        self.record(Call::Tokenize(hash(request.as_bytes()), request.len()));
        self.inner.tokenize(tokenizer, request)
    }
    fn attempt_provider(
        &mut self,
        provider: &str,
        model: &str,
        request: &str,
        remaining: u64,
        sink: &mut ProviderSink,
    ) -> ProviderAttempt {
        assert_eq!(provider, "fake.local");
        assert_eq!(model, "fake-basic");
        self.record(Call::Provider(
            hash(request.as_bytes()),
            request.len(),
            remaining,
        ));
        self.inner
            .attempt_provider(provider, model, request, remaining, sink)
    }
    fn invoke_tool(
        &mut self,
        call_id: &str,
        tool: &str,
        arguments: &str,
        sink: &mut ToolResultSink,
    ) -> bool {
        assert_eq!(tool, "fixture.read");
        assert_eq!(arguments, "{\"query\":\"alpha\"}");
        self.record(Call::Tool(
            hash(call_id.as_bytes()),
            hash(arguments.as_bytes()),
        ));
        self.inner.invoke_tool(call_id, tool, arguments, sink)
    }
    fn invoke_tool_with_deadline(
        &mut self,
        call_id: &str,
        tool: &str,
        arguments: &str,
        remaining: u64,
        sink: &mut ToolResultSink,
    ) -> bool {
        self.record(Call::ToolDeadline(remaining));
        // Preserve the original AgentHost default's one direct delegation.
        self.invoke_tool(call_id, tool, arguments, sink)
    }
}
fn equal_runs(baseline: &AgentRun, current: &AgentRun) {
    assert_eq!(baseline.status(), current.status());
    assert_eq!(baseline.final_message(), current.final_message());
    assert_eq!(baseline.trace(), current.trace());
    assert_eq!(baseline.trace_digest(), current.trace_digest());
    assert_eq!(baseline.evidence(), current.evidence());
    assert_eq!(baseline.evidence_digest(), current.evidence_digest());
    assert_eq!(baseline.accounting_receipt(), current.accounting_receipt());
    assert_eq!(
        baseline.accounting_receipt_digest(),
        current.accounting_receipt_digest()
    );
}

#[test]
fn borrowed_private_host_keeps_complete_fixture_outputs_call_order_and_receiver_drop_cost() {
    let profile = fixture_profile();
    let task = fixture_task();
    let mut plain = Agent::new(&profile, FakeHost::fixture(), AgentCancellation::new()).unwrap();
    let log = RefCell::new(Log::new());
    let mut recorded = Agent::new(&profile, Recorded::new(&log), AgentCancellation::new()).unwrap();
    let baseline = plain.run(&task).unwrap();
    let current = recorded.run(&task).unwrap();
    equal_runs(&baseline, &current);
    assert_eq!(current.status(), RunStatus::Completed);
    assert_eq!(current.final_message(), Some("done"));
    replay_trace(current.trace()).unwrap();
    replay_evidence(current.evidence(), &profile, &current).unwrap();
    replay_accounting_receipt(current.accounting_receipt(), &current).unwrap();
    let calls = log.borrow();
    // Pure observations are retained in the fixed log. These are the exact
    // semantic host dispatches, including the positive tool deadline hook.
    let mut semantic = [Call::Empty; 8];
    let mut count = 0;
    for call in calls.entries() {
        if matches!(
            call,
            Call::Tokenize(..) | Call::Provider(..) | Call::ToolDeadline(..) | Call::Tool(..)
        ) {
            assert!(count < semantic.len());
            semantic[count] = *call;
            count += 1;
        }
    }
    // The retained fixture first renders a 4096-token output cap, then
    // shrinks it to context minus request tokens. Both caps have four decimal
    // digits, so the second rendering has the same byte/token count and fits.
    assert_eq!(count, 8);
    let (
        Call::Tokenize(first_initial, first_initial_len),
        Call::Tokenize(first, first_len),
        Call::Provider(first_sent, sent_len, first_deadline),
        Call::ToolDeadline(tool_deadline),
        Call::Tool(_, arguments),
        Call::Tokenize(second_initial, second_initial_len),
        Call::Tokenize(second, second_len),
        Call::Provider(second_sent, second_sent_len, second_deadline),
    ) = (
        semantic[0],
        semantic[1],
        semantic[2],
        semantic[3],
        semantic[4],
        semantic[5],
        semantic[6],
        semantic[7],
    )
    else {
        panic!("provider/tool dispatch order changed")
    };
    assert_eq!(first_initial_len, first_len);
    assert_eq!(second_initial_len, second_len);
    assert_ne!(first_initial, first);
    assert_ne!(second_initial, second);
    assert_eq!((first, first_len), (first_sent, sent_len));
    assert_eq!((second, second_len), (second_sent, second_sent_len));
    assert_ne!(first, second);
    assert_eq!(arguments, hash(b"{\"query\":\"alpha\"}"));
    assert_eq!(
        (first_deadline, tool_deadline, second_deadline),
        (1000, 1000, 1000)
    );
    drop(calls);
    drop(baseline);
    drop(current);
    drop(plain);
    drop(recorded);

    // Host/profile setup is outside the window. Both complete AgentRun
    // receivers, replay products and returned byte buffers drop inside it.
    let mut plain = Agent::new(&profile, FakeHost::fixture(), AgentCancellation::new()).unwrap();
    let log = RefCell::new(Log::new());
    let mut recorded = Agent::new(&profile, Recorded::new(&log), AgentCancellation::new()).unwrap();
    let ((), baseline) = measure(|| {
        let result = plain.run(&task).unwrap();
        assert_eq!(result.status(), RunStatus::Completed);
        drop(result);
    });
    let ((), observed) = measure(|| {
        let result = recorded.run(&task).unwrap();
        assert_eq!(result.status(), RunStatus::Completed);
        drop(result);
    });
    assert_eq!(baseline.allocated, observed.allocated);
    assert_eq!(baseline.allocations, observed.allocations);
    assert_eq!(baseline.reallocations, observed.reallocations);
    assert_eq!(baseline.highwater, observed.highwater);
    for metrics in [baseline, observed] {
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}

/// Complete small pre-erasure deadline algorithm, independent of the current
/// dynamic helper. This does not duplicate routing or the execution engine.
fn saved_deadline<H: AgentHost>(
    profile: &Profile,
    host: &H,
    cancellation: &AgentCancellation,
    epoch: u64,
) -> Result<u64, Termination> {
    let termination = if cancellation.is_cancelled() {
        Some(operational("SPX-I220", "Agent Runtime run was cancelled"))
    } else if host.elapsed_ms() >= profile.limits.max_elapsed_ms {
        Some(operational(
            "SPX-I221",
            "Agent Runtime deadline was exceeded",
        ))
    } else if host.policy_epoch() != epoch {
        Some(g207("policy revoked"))
    } else {
        None
    };
    if let Some(diagnostic) = termination {
        return Err(super::super::private::termination_for_host_erasure_test(
            diagnostic,
        ));
    }
    profile
        .limits
        .max_elapsed_ms
        .checked_sub(host.elapsed_ms())
        .filter(|remaining| *remaining > 0)
        .ok_or_else(|| {
            super::super::private::termination_for_host_erasure_test(operational(
                "SPX-I221",
                "Agent Runtime deadline was exceeded",
            ))
        })
}

#[test]
fn borrowed_deadline_dispatch_preserves_complete_generic_observation_priority_without_allocating() {
    let profile = parse_profile(&fixture_profile()).unwrap();
    for (cancelled, elapsed, epoch) in [
        (false, 0, 7),
        (false, 999, 7),
        (false, 1000, 7),
        (false, 1001, 8),
        (false, 0, 8),
        (true, 1000, 8),
    ] {
        let old_log = RefCell::new(Log::new());
        let current_log = RefCell::new(Log::new());
        let old = Recorded::new(&old_log);
        let current = Recorded::new(&current_log);
        for host in [&old, &current] {
            host.inner.probe.elapsed.set(elapsed);
            host.inner.probe.epoch.set(epoch);
        }
        let cancellation = AgentCancellation::new();
        if cancelled {
            cancellation.cancel();
        }
        let old_result = saved_deadline(&profile, &old, &cancellation, 7);
        let current_result = super::super::private::deadline_for_host_erasure_test(
            &profile,
            &current,
            &cancellation,
            7,
        );
        match (&old_result, &current_result) {
            (Ok(old), Ok(current)) => assert_eq!(old, current),
            (Err(old), Err(current)) => {
                assert_eq!(old.status, current.status);
                assert_eq!(old.code, current.code);
                assert_eq!(old.message, current.message);
            }
            _ => panic!("complete deadline result diverged"),
        }
        assert_eq!(old_log.borrow().entries(), current_log.borrow().entries());
        if !cancelled && elapsed < 1000 && epoch == 7 {
            let (result, metrics) = measure(|| {
                // Only borrowed references are converted at this boundary.
                let host: &dyn AgentHost = &current;
                super::super::private::deadline_for_host_erasure_test(
                    &profile,
                    host,
                    &cancellation,
                    7,
                )
                .unwrap_or_else(|_| panic!("original positive deadline fixture was refused"))
            });
            assert_eq!(result, 1000 - elapsed);
            assert_eq!(metrics.allocated, 0);
            assert_eq!(metrics.allocations, 0);
            assert!(!metrics.invalid && metrics.live == 0);
        }
    }
}
