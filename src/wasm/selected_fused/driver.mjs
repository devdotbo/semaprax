// Owning driver for the selected Core-Wasm FusedConstructionV1 executor.
// Authored; execution is pending the root gate run. Each process holds the
// realm's single namespace, so namespace-dimension controls use their own
// mode. The module under test is the UNVALIDATED precursor lowering of the
// actual selected function; nothing here is a publishable artifact.
import fs from 'node:fs';
import assert from 'node:assert/strict';
import {createAccount, createAuthorizedHost, createSharedNamespace, SelectedRefusal} from './host.mjs';

const [wasmPath, digest, rowsText, widthText, metadataText, rowBytesText, accountText, registryText,
  orderText, textText, mode, expectedText] = process.argv.slice(2);
const bytes = fs.readFileSync(wasmPath);
const grant = Object.freeze({digest, rows: Number(rowsText), width: Number(widthText),
  metadataBytes: Number(metadataText), rowBytes: Number(rowBytesText), accountBytes: Number(accountText),
  registryEntries: Number(registryText), order: orderText.split(',')});
const rows = grant.rows, width = grant.width, extent = rows * width;
const R = BigInt(rows), W = BigInt(width), textBytes = Number(textText);
const expected = BigInt(expectedText);
const LITERAL = 1024;
const isRefusal = code => error => error instanceof SelectedRefusal && error.code === code;
const refusal = (operation, code) => assert.throws(operation, isRefusal(code));
let observed = null;
const capture = code => error => { observed = error.observation; return isRefusal(code)(error); };
const exact = () => createAccount(grant.accountBytes);

// A complete successful guest invocation: one backing, exact charges, Strings
// ordinary, every child settled, every entry retired and precharge released.
function success(result) {
  const o = result.observation, [domain] = o.domains;
  assert.equal(result.status, 0);
  assert.equal(result.value, expected);
  assert.equal(o.selected, null);
  assert.equal(o.domains.length, 1);
  assert.equal(domain.phase, 'Settled');
  assert.equal(domain.failure, null);
  assert.equal(domain.allocations, 1);
  assert.equal(domain.releases, 1);
  assert.equal(domain.payloadBytes, extent);
  assert.equal(domain.metadataBytes, grant.metadataBytes);
  assert.equal(domain.storageBytes, rows * grant.rowBytes);
  assert.equal(domain.issued, rows);
  assert(domain.peakViews >= 1 && domain.peakViews <= 64);
  assert.equal(o.fusedEvents, 1);
  assert.equal(o.accountCarved, grant.accountBytes);
  assert.equal(o.strings, rows);
  assert.equal(o.stringDrops, rows);
  assert.equal(o.stringBytes, rows * textBytes);
  assert.equal(o.liveEntries, 0);
  assert.equal(o.outstandingQuota, 0);
  assert(o.nextSequence > o.firstSequence);
  return o;
}
// The guest's own canonical construction, driven through the same imports.
function build(c) {
  const domain = c.start(R, W);
  let vector = c.vec_new(domain, R);
  for (let row = 0; row < rows; row++) {
    const text = c.string_literal(domain, LITERAL, textBytes);
    const lease = c.issue(domain, BigInt(row));
    vector = c.row_push(domain, vector, BigInt(row), text, lease);
  }
  c.complete(domain);
  return [domain, vector];
}
function settledControl(instance, body, failure = null) {
  const result = instance.session({account: exact()}, body), [domain] = result.observation.domains;
  assert.equal(domain.phase, 'Settled');
  assert.equal(domain.failure, failure);
  assert.equal(result.observation.liveEntries, 0);
  assert.equal(result.observation.outstandingQuota, 0);
  return result.observation;
}

if (mode === 'original') {
  assert.equal(rows, 4096);
  assert.equal(extent, 12288);
  assert.equal(rows * textBytes, 12288);
  assert.equal(expected, 8386560n);
  const namespace = createSharedNamespace({registryEntries: 65536});
  const instance = await createAuthorizedHost(namespace).instantiate(bytes, grant);
  // Reentry: the namespace sequence continues and never resets.
  let previous = 0n;
  const account = exact();
  for (let run = 0; run < 3; run++) {
    const o = success(instance.invoke({account}));
    assert.equal(o.events, 1);
    assert.equal(o.cumulative, 12288);
    assert(o.firstSequence > previous);
    previous = o.nextSequence - 1n;
  }
} else if (mode === 'controls') {
  assert.deepEqual(grant.order, ['scalar', 'text', 'payload']);
  // Room for exactly two concurrent domains' precharge, plus one entry.
  const namespace = createSharedNamespace({registryEntries: 2 * grant.registryEntries + 1});
  refusal(() => createSharedNamespace(), 'namespace-singleton');
  refusal(() => createAuthorizedHost(Object.freeze({observe: namespace.observe})), 'namespace');
  const host = createAuthorizedHost(namespace);
  const tampered = new Uint8Array(bytes);
  tampered[tampered.length - 1] ^= 1;
  await assert.rejects(host.instantiate(tampered, grant), isRefusal('module-digest'));
  for (const key of ['metadataBytes', 'accountBytes', 'registryEntries']) {
    await assert.rejects(host.instantiate(bytes, {...grant, [key]: grant[key] - 1}), isRefusal('grant'));
  }
  const instance = await host.instantiate(bytes, grant, {controls: true});

  // Physical account: exact and +1 succeed; one short refuses at the first
  // Bytes operand, after that row's String, before any payload allocation.
  success(instance.invoke({account: exact()}));
  const plus = success(instance.invoke({account: createAccount(grant.accountBytes + 1)}));
  assert.equal(plus.accountBytes, grant.accountBytes + 1);
  assert.throws(() => instance.invoke({account: createAccount(grant.accountBytes - 1)}), capture('metadata'));
  assert.equal(observed.domains[0].phase, 'Abandoned');
  assert.equal(observed.domains[0].allocations, 0);
  assert.equal(observed.domains[0].metadataBytes, 0);
  assert.equal(observed.accountCarved, rows * grant.rowBytes);
  assert.equal(observed.events, 0);
  assert.equal(observed.cumulative, 0);
  assert.equal(observed.strings, 1);
  assert.equal(observed.liveEntries, 0);
  assert.equal(observed.outstandingQuota, 0);
  refusal(() => instance.invoke({}), 'account');

  // Ordinary limits: 31 ordinary events plus the fused event fit; 32 plus
  // one refuse with nothing carved for metadata and no event charged.
  assert.equal(success(instance.invoke({account: exact(), ordinaryEvents: 31})).events, 32);
  assert.throws(() => instance.invoke({account: exact(), ordinaryEvents: 32}), capture('ordinary-byte-capacity'));
  assert.equal(observed.domains[0].allocations, 0);
  assert.equal(observed.domains[0].metadataBytes, 0);
  assert.equal(observed.events, 32);
  success(instance.invoke({account: exact(), ordinaryBytes: 2097152 - extent}));
  assert.throws(() => instance.invoke({account: exact(), ordinaryBytes: 2097152 - extent + 1}),
    capture('ordinary-byte-capacity'));
  assert.equal(observed.domains[0].allocations, 0);

  // Partial faults at row cuts: the plan's exact push failure exit, the
  // selected status first and sticky, one release, nothing refunded.
  const cuts = rows <= 64 ? [...Array(rows).keys()] : [0, 1, 2, rows >> 1, rows - 2, rows - 1];
  for (const cut of cuts) {
    const result = instance.invoke({account: exact(), faults: {push: cut}}), o = result.observation;
    assert.equal(result.status, 15);
    assert.equal(o.selected, 15);
    assert.equal(result.value, undefined);
    assert.equal(o.domains[0].phase, 'Settled');
    assert.equal(o.domains[0].allocations, 1);
    assert.equal(o.domains[0].releases, 1);
    assert.equal(o.events, 1);
    assert.equal(o.cumulative, extent);
    assert.equal(o.strings, cut + 1);
    assert.equal(o.stringDrops, cut + 1);
    assert.equal(o.liveEntries, 0);
  }
  const vec = instance.invoke({account: exact(), faults: {vec: true}});
  assert.equal(vec.status, 15);
  assert.equal(vec.observation.domains[0].allocations, 0);
  assert.equal(vec.observation.events, 0);
  assert.equal(vec.observation.accountCarved, 0);
  // Physical payload failure is fail-stop: carving and charges precede it.
  assert.throws(() => instance.invoke({account: exact(), faults: {payload: true}}),
    error => { observed = error.observation; return error.message === 'selected-fused:physical-payload-fault'; });
  assert.equal(observed.domains[0].phase, 'Abandoned');
  assert.equal(observed.domains[0].allocations, 0);
  assert.equal(observed.accountCarved, grant.accountBytes);
  assert.equal(observed.events, 1);
  assert.equal(observed.liveEntries, 0);
  success(instance.invoke({account: exact()}));

  // Foreign host and foreign instance: a live token from either is refused in
  // a fresh domain, and its owner is unaffected and still publishes.
  const otherHost = createAuthorizedHost(namespace);
  const third = await host.instantiate(bytes, grant, {controls: true});
  const strangers = [[await otherHost.instantiate(bytes, grant, {controls: true}), 'foreign-host'],
    [await host.instantiate(bytes, grant, {controls: true}), 'foreign-instance']];
  for (const [stranger, code] of strangers) {
    const outer = instance.session({account: exact()}, a => {
      const da = a.start(R, W), va = a.vec_new(da, R);
      const ta = a.string_literal(da, LITERAL, textBytes), oa = a.issue(da, 0n);
      const inner = stranger.session({account: exact()}, b => {
        const db = b.start(R, W);
        // Two domains' precharge are held; a third cannot be precharged.
        assert.throws(() => third.session({account: exact()}, c => c.start(R, W)), isRefusal('registry-exhausted'));
        refusal(() => b.vec_new(da, R), code);
        const vb = b.vec_new(db, R), tb = b.string_literal(db, LITERAL, textBytes), ob = b.issue(db, 0n);
        for (const [left, right] of [[da, db], [va, vb], [ta, tb], [oa, ob]]) assert.notEqual(left, right);
        refusal(() => b.row_push(db, vb, 0n, tb, oa), code);
        refusal(() => b.row_push(db, vb, 0n, tb, ob), code);
        b.abort(db);
        refusal(() => b.owner_drop(db, oa), code);
        // Same registry entry, next sequence: never minted, so stale.
        refusal(() => b.owner_drop(db, ob + (1n << 20n)), 'stale');
        b.owner_drop(db, ob);
        b.string_drop(db, tb);
        b.vec_drop(db, vb);
        b.settle_abort(db);
        return 0n;
      });
      assert.equal(inner.observation.domains[0].phase, 'Settled');
      assert.equal(inner.observation.domains[0].failure, code);
      assert.equal(inner.observation.domains[0].releases, 1);
      const renewed = a.row_push(da, va, 0n, ta, oa);
      a.abort(da);
      a.vec_drop(da, renewed);
      a.settle_abort(da);
      return 0n;
    });
    assert.equal(outer.observation.domains[0].phase, 'Settled');
    assert.equal(outer.observation.domains[0].failure, null);
    assert.equal(outer.observation.liveEntries, 0);
  }
  // One held account and one invocation per instance at a time.
  const held = exact();
  instance.session({account: held}, () => {
    assert.throws(() => third.session({account: held}, () => 0n), isRefusal('account-held'));
    assert.throws(() => instance.session({account: exact()}, () => 0n), isRefusal('reentrant-invocation'));
    return 0n;
  });
  // Foreign invocation: every token of a finished invocation is retired.
  let retired;
  settledControl(instance, c => {
    retired = c.start(R, W);
    c.abort(retired);
    c.settle_abort(retired);
    return 0n;
  });
  instance.session({account: exact()}, c => { refusal(() => c.vec_new(retired, R), 'stale'); return 0n; });
  // The first selected failure is sticky across later selections.
  const sticky = instance.session({account: exact()}, c => {
    c.select_failure(15);
    c.select_failure(1);
    c.return_failure();
    return 0n;
  });
  assert.equal(sticky.status, 15);
  assert.equal(sticky.value, undefined);
  // Escape: an unsettled child at invocation end publishes nothing.
  assert.throws(() => instance.session({account: exact()}, c => { build(c); return 1n; }),
    capture('unsettled-escape'));
  assert.equal(observed.domains[0].phase, 'Abandoned');
  assert.equal(observed.domains[0].releases, 1);
  assert.equal(observed.liveEntries, 0);
  assert.equal(observed.outstandingQuota, 0);

  // Popped child survives its parent Vec; store renews; deep clone keeps the
  // ordinary event and width counters and is isolated; the last child
  // releases the backing exactly once; no charge is refunded.
  const popped = settledControl(instance, c => {
    let [domain, vector] = build(c), row;
    [row, vector] = c.vec_pop(domain, vector);
    c.vec_drop(domain, vector);
    const view = c.row_borrow(domain, row);
    assert.equal(c.view_byte(domain, view, 0n), 0);
    assert.equal(c.view_byte(domain, view, W), -1);
    c.end_borrow(domain, view);
    c.store(domain, row, 0, 7);
    const clone = c.deep_clone(domain, row);
    assert.equal(c.bytes_byte(domain, clone, 0), 7);
    c.store(domain, row, 0, 9);
    assert.equal(c.bytes_byte(domain, clone, 0), 7);
    const again = c.row_borrow(domain, row);
    assert.equal(c.view_byte(domain, again, 0n), 9);
    c.end_borrow(domain, again);
    c.bytes_drop(domain, clone);
    c.row_drop(domain, row);
    return 0n;
  });
  assert.equal(popped.domains[0].releases, 1);
  assert.equal(popped.events, 2);
  assert.equal(popped.fusedEvents, 1);
  assert.equal(popped.cumulative, extent + width);
  assert.equal(popped.strings, rows);
  assert.equal(popped.stringDrops, rows);

  // Sticky domain failure; cleanup remains possible and cannot replace it.
  settledControl(instance, c => {
    let [domain, vector] = build(c), row;
    [row, vector] = c.vec_pop(domain, vector);
    const view = c.row_borrow(domain, row);
    refusal(() => c.store(domain, row, 0, 1), 'borrowed');
    refusal(() => c.vec_len(domain, vector), 'borrowed');
    c.end_borrow(domain, view);
    c.row_drop(domain, row);
    c.vec_drop(domain, vector);
    return 0n;
  }, 'borrowed');
  const views = settledControl(instance, c => {
    const [domain, vector] = build(c), open = [];
    for (let slot = 0; slot < 64; slot++) open.push(c.payload_borrow(domain, vector, BigInt(slot % rows)));
    refusal(() => c.payload_borrow(domain, vector, 0n), 'view-capacity');
    for (const view of open) c.end_borrow(domain, view);
    c.vec_drop(domain, vector);
    return 0n;
  }, 'view-capacity');
  assert.equal(views.domains[0].peakViews, 64);
  for (const [operation, code] of [
    [(c, d, v) => { const m = c.vec_move(d, v); refusal(() => c.vec_len(d, v), 'stale'); return m; }, 'stale'],
    [(c, d, v) => { const s = c.sort(d, v); refusal(() => c.vec_len(d, v), 'stale'); return s; }, 'stale'],
    [(c, d, v) => {
      const view = c.payload_borrow(d, v, 0n);
      refusal(() => c.vec_move(d, v), 'borrowed');
      refusal(() => c.vec_drop(d, v), 'borrowed');
      c.end_borrow(d, view);
      return v;
    }, 'borrowed'],
    [(c, d, v) => {
      const view = c.payload_borrow(d, v, 0n);
      c.end_borrow(d, view);
      refusal(() => c.view_byte(d, view, 0n), 'stale');
      return v;
    }, 'stale'],
    [(c, d, v) => { refusal(() => c.field_scalar(d, v, R), 'range'); return v; }, 'range'],
  ]) {
    settledControl(instance, c => {
      const [domain, vector] = build(c);
      c.vec_drop(domain, operation(c, domain, vector));
      return 0n;
    }, code);
  }
  // Construction order and phase refusals.
  settledControl(instance, c => {
    const d = c.start(R, W), v = c.vec_new(d, R);
    refusal(() => c.issue(d, 0n), 'operand-order');
    c.abort(d); c.vec_drop(d, v); c.settle_abort(d);
    return 0n;
  }, 'operand-order');
  settledControl(instance, c => {
    const d = c.start(R, W), v = c.vec_new(d, R), t = c.string_literal(d, LITERAL, textBytes);
    refusal(() => c.issue(d, 1n), 'issue-range');
    c.abort(d); c.string_drop(d, t); c.vec_drop(d, v); c.settle_abort(d);
    return 0n;
  }, 'issue-range');
  settledControl(instance, c => {
    const d = c.start(R, W), v = c.vec_new(d, R), t = c.string_literal(d, LITERAL, textBytes);
    const o = c.issue(d, 0n);
    refusal(() => c.issue(d, 1n), 'unpublished');
    refusal(() => c.owner_drop(d, o), 'phase');
    c.abort(d);
    refusal(() => c.settle_abort(d), 'unsettled');
    c.owner_drop(d, o); c.string_drop(d, t); c.vec_drop(d, v); c.settle_abort(d);
    return 0n;
  }, 'unpublished');
} else if (mode === 'exhaustion') {
  // Sequence exhaustion refuses before mutation; the namespace never resets.
  const namespace = createSharedNamespace({registryEntries: grant.registryEntries, sequenceLimit: 3});
  const host = createAuthorizedHost(namespace);
  const exhausted = await host.instantiate(bytes, grant, {controls: true});
  const spent = exhausted.session({account: exact()}, c => {
    const d = c.start(R, W), v = c.vec_new(d, R), t = c.string_literal(d, LITERAL, textBytes);
    refusal(() => c.issue(d, 0n), 'namespace-exhausted');
    c.abort(d); c.string_drop(d, t); c.vec_drop(d, v); c.settle_abort(d);
    return 0n;
  });
  assert.equal(spent.observation.domains[0].phase, 'Settled');
  assert.equal(spent.observation.domains[0].allocations, 0);
  assert.equal(spent.observation.events, 0);
  assert.equal(spent.observation.domains[0].metadataBytes, 0);
  assert.equal(spent.observation.liveEntries, 0);
  assert.throws(() => exhausted.invoke({account: exact()}), isRefusal('namespace-exhausted'));
  const later = await createAuthorizedHost(namespace).instantiate(bytes, grant);
  assert.throws(() => later.invoke({account: exact()}), isRefusal('namespace-exhausted'));
  assert.equal(namespace.observe().nextSequence, 4n);
} else {
  throw new Error(`unknown selected driver mode ${mode}`);
}
console.log('selected-fused-wasm-v1-ok');
