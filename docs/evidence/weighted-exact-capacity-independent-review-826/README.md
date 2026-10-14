# Independent bounded OPT826 source review and closure map

Status: completed source review; runtime qualification pending.
Audience: root qualification and closure lane.

One freshly authorized tool-disabled Sonnet review inspected fix97480a13ecbefdc58d4bb47b4b37dd14a82f5063. The exact whitelist/payload SHA, durable120-second/768MiB/2MiB runner, raw result, extracted review, and actual receipt are retained. The returned model was claude-sonnet-5-5, one turn, exit0,74.6133158340017 seconds, sampled process-group peakRSS287571968 bytes. Provider list costUSD0.165064; actual billing remains unknown. No savings, automatic approval, runtime qualification, concurrent process, retry, Cargo/build/test, root source edit, or GitHub write is claimed.

Raw verdict: no production blocker. Explicit Vec::with_capacity plus unchanged ordered extend fixes the inherited small-collector capacity concern while reserving tuple slots and the separate Rc header before their respective allocations. The direct helper control observes actual capacity and exact requested backing at widths0..4, complete Drop and exact/short/zero/sticky refusals. Physical sharing charges remain different from complete old copying charges; algebra and work are compared independently.

The raw review is unchanged. Its omitted-context claims were assessed in one bounded source read, saved with exact source lines and SHAs in local-source-assessment.json:

- `Automaton::node` always pushes a new node; `movement` always appends an edge; `end` constructs a fresh node then adds its terminal edge to the common sink. Consequently, widths1..4 in the full receiver test do produce distinct successor target keys. The reviewer's hypothetical dedup collapse does not occur.
- At width>=2 and u64MAX, the next Failure-labelled step merges those distinct targets into the same sink. The second successor slot.checked_add(MAX) overflows. The raw claim that checked_add overflow is not exercised is refuted by the actual source.
- Width0 is a no-successor integration automaton; the direct producer test independently covers an actual empty target map. Physical integration at width2/3 could strengthen coverage, but is not a demonstrated production defect.
- PhysicalReservation.reserve preserves a selected sticky refusal before accepting even a zero-byte request. No hidden reset occurs.

closure-map-826.json lists all seven canonical_shared exact bodies plus the full original-source custody owner. All eight are now registered in root's frozen350-body successor at a184d; registered source SHA values were verified against that commit. Their registration is not runtime evidence. Close826 only after those current-source bodies execute successfully and the qualified implementation is verified pushed to wavect/v0-10-0 / PR761. No original receiver or inherited old behavior is removed or relabeled. The issue's BTreeSet wording is a minor description mismatch; actual input is BTreeMap<usize,u64>.
