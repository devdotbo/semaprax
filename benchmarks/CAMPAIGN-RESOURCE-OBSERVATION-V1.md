# Campaign resource observation v1

Owns OPT #894. This is benchmark host observation, not compiler/application
admission or a proof of causal isolation. It applies symmetrically to calibration
and each attempted task through the existing `campaign_resources.guarded_attempt`.
The existing CLI49, ShiftSim15 and web912 acceptance requirements, prompts,
attempt/cost denominators and resource receipt schema remain unchanged.

## Disk and host fields

All original disk receipt fields and incidents remain. `disk_eligible` means
only that the original disk/monitor incident list is empty. A new `host_samples`
array and `host_coverage` object retain process and memory observations, monotonic
sampling times, intervals, omissions, probe failures and policy violations.
`clean_comparison_eligible` now additionally requires complete point-sampling
coverage under this declared policy. It does **not** establish causal isolation:
`causal_isolation_established` is always false and the basis explicitly says so.
Historical receipts are not relabelled. Old disk-only flags cannot be interpreted
as this new coverage. Existing campaign summaries receive the stricter flag;
no campaign/acceptance algorithm or historical transcript is rewritten.

The observer reads process PID, parent PID, process group, reported RSS and
executable name using `ps`, without argv, environment, usernames or working
directories. Only executable basenames are retained. Each snapshot records a
SHA/length of the probe bytes, process counts, observer PID, summed reported RSS
of its parent-linked descendants and at most32 known competing-heavy records
(with an explicit omitted count). Raw process listings and path prefixes are not
written to receipts. RSS sums can count shared pages repeatedly and are not
physical-memory attribution. PID ancestry is a snapshot, not a lifetime seal;
reparented, short-lived or unrecognized workloads can escape observation.

The fixed known-heavy set is cargo, rustc, clippy-driver, clang/clang++, gcc/g++,
cc1/cc1plus, ld/lld, ninja, make and cmake. Their presence outside the observer's
current descendant set is a policy violation. An expected compiler run inside
the measured campaign is not mistaken for a competing job. This is a finite
executable-name census, not universal discovery of every heavy Python/Node/app
workload. Visible process namespaces do not establish machine-wide visibility.

Linux requires consistent `MemTotal` and `MemAvailable` from `/proc/meminfo`;
`MemFree` is not a substitute. Darwin requires exactly one reported
`System-wide memory free percentage` from `memory_pressure -Q`; this field is
not translated into a kernel pressure-state label. Unavailable, malformed,
failed or unsupported probes remain unknown. The policy records a2GiB minimum
Linux available value,10% minimum Darwin reported free percentage and8GiB maximum
summed owned reported RSS. These observation thresholds are not a compiler cap
or a strict operating-system memory limit.

## Bounds, admission and settlement

Each owned probe has a3-second deadline, at most1MiB stdout, strict parsing and
bounded cleanup wait. Only that newly spawned read-only probe can be killed on
a probe failure. No campaign, unrelated task, PID tree or machine-wide process
is signalled. Disk admission remains the original5GiB floor. Known initial
heavy-job/memory/RSS violations refuse **before** invoking the attempted task
and retain a `refused_host_resources` receipt with the actual observations;
no reserve file or paid launch is claimed. Unknown initial coverage does not
invent a known pressure event or compiler failure; an attempt can proceed, but
it can never earn clean-comparison eligibility with missing coverage.

The observer samples initially, every5 seconds, and after joining its watcher
at finish. Receipts retain observed monotonic intervals. At least two complete
samples, no backwards/invalid intervals and no interval or endpoint gap over15
seconds are required. At most4096 samples are retained; further samples are
counted as omitted and coverage becomes unknown. This is bounded point sampling,
not continuous observation: transient activity between samples remains unknown.

A later host-policy violation marks resource contamination and disqualifies the
comparison; incomplete/unsupported coverage is unknown, not a clean sample.
Original status, acceptance, paid usage and cost stay intact after dispatch.
There is no retry, acceptance waiver, free failed task or fabricated billing.
The observer joins its own watcher and cleans its own emergency reserve using
the original lifecycle. It does not observe settlement of all descendant tasks;
existing process-group/outer guards must establish that separately if required.
Use the existing coordinated external heavy-job admission and hard resource
guards before real paid campaigns. Neither this observer nor a clean sampled
policy flag justifies an intrinsic compiler/language speedup or token claim.

## Focused qualification

Run the existing resource fixture module once after the complete change:

```sh
python3 -m unittest -v test_campaign_resources
```

Run from `benchmarks/`. Fixtures preserve the complete original disk, failure,
reserve and receipt assertions with a deterministic healthy-host observation.
Additive fixtures cover malformed/duplicate process metadata, own descendants
versus competing jobs, both memory parsers, bounded owned probe output/timeout/
exit, unsupported/missing observations, initial refusal, later contamination,
monotonic gaps/drift, sample omissions and acceptance/cost preservation.
They launch only small local Python fixture probes, never a model, compiler,
application, package installation or paid campaign. Passing fixtures is not
actual host/campaign qualification or benchmark evidence.
