# Independent bounded weighted canonical source review

Status: completed source review; probable inherited accounting concern, no execution.
Audience: root integration and qualification lane.

This separate explicitly authorized review is pinned to frozen93fdc103c89fda0230538e568b3f8f661c55f399, including canonical sharing1dad6f08b7ea8da7fe18e3cdd9838f648a0211cd. Exact production source, complete old receiver, owning tests, source whitelist, payload SHA, durable runner, raw result and extracted findings are retained. No constructor retry or concurrent Claude process occurred.

Claude's returned canonical model was claude-sonnet-5-5. One tool-disabled turn completed in85.4892628330017 seconds, exit0, sampled peak process-group RSS324075520 bytes. The120-second/768MiB/2MiB guards were not reached. Provider list cost wasUSD0.189448; actual billing remains unknown. No savings or approval is claimed.

The reviewer found no proven semantic/work regression and accepted value-based Rc ordering and ordinary Drop. Its main concern is probable inherited Vec capacity underfunding: the successor targets reserve len*size_of(element), then collect a BTreeMap iterator into Vec, which may have a minimum capacity greater than len for1–3 elements. Both old and new receivers use that path, but the new comment claims complete allocated backing. The reviewer recommends explicit with_capacity(len) plus extend, with payment before allocation, and notes whole-probe BTree pessimism can hide a component undercharge. This is an unverified standard-library behavior concern, not a local runtime result.

Raw additional uncertainties and test gaps are preserved in review.md: payer gating and clone cursor charge differences, mirrored Rc layout, unseen error helper, cycle-error assertions, intermediate refusal/overflow coverage, and active-payer parity. Those source omissions are not asserted as proven defects here. No Cargo, builds, compiler, runtime, source edits or GitHub writes occurred. Root received raw findings before this evidence commit.
