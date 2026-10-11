// Owning driver for the selected Core-Wasm FusedConstructionV1 executor.
// Authored; execution is pending the root gate run. The module under test is
// the publishable artifact `accept` emitted from the ORIGINAL 4096-row source
// through `validate_selected`. Each process holds the realm's single
// namespace, so namespace-dimension controls run as their own mode.
import fs from 'node:fs';
import assert from 'node:assert/strict';
import {createAccount, createAuthorizedHost, createSharedNamespace} from './host.mjs';

const [wasmPath, digest, rowsText, widthText, metadataText, rowBytesText, accountText, registryText,
  orderText, textText, mode, expectedText] = process.argv.slice(2);
const bytes = fs.readFileSync(wasmPath);
const grant = Object.freeze({digest, rows: Number(rowsText), width: Number(widthText),
  metadataBytes: Number(metadataText), rowBytes: Number(rowBytesText), accountBytes: Number(accountText),
  registryEntries: Number(registryText), order: orderText.split(',')});
const rows = grant.rows, width = grant.width, extent = rows * width;
const registryBytes = grant.registryEntries * (8 + 4 * 6 + 1);
const registryReservation = Math.ceil(registryBytes / 8) * 8;
const R = BigInt(rows), W = BigInt(width), textBytes = Number(textText);
const expected = BigInt(expectedText);
const LITERAL = 1024;
// By name and code: refusals from separately loaded module copies are
// distinct classes but carry the same identity.
const isRefusal = code => error => error?.name === 'SelectedRefusal' && error.code === code;
const refusal = (operation, code) => assert.throws(operation, isRefusal(code));
let observed = null;
const capture = code => error => { observed = error.observation; return isRefusal(code)(error); };
let namespace = null;
const exact = () => createAccount(namespace, grant.accountBytes);

// A complete successful guest invocation: one backing, exact physical debits,
// Strings ordinary, every child settled, every entry retired.
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
  assert.equal(o.registryDebit, registryBytes);
  assert.equal(o.accountUsed, grant.accountBytes);
  assert.equal(o.strings, rows);
  assert.equal(o.stringDrops, rows);
  assert.equal(o.stringBytes, rows * textBytes);
  assert.equal(o.liveEntries, 0);
  assert.equal(o.outstandingQuota, 0);
  assert(o.nextSequence > o.firstSequence);
  if (o.finalizers !== undefined) {
    const contained = o.finalizers.filter(event => event.container === true).map(event => event.kind);
    assert.deepEqual(contained, Array.from({length: rows}, () => grant.order.filter(role => role !== 'scalar')).flat());
  }
  return o;
}
// The guest's own canonical construction, through the same imports. Returns
// the last row's String and owner tokens as well.
function build(c) {
  const domain = c.start(R, W);
  let vector = c.vec_new(domain, R), text = 0n, lease = 0n;
  for (let row = 0; row < rows; row++) {
    text = c.string_literal(domain, LITERAL, textBytes);
    lease = c.issue(domain, BigInt(row));
    vector = c.row_push(domain, vector, BigInt(row), text, lease);
  }
  c.complete(domain);
  return [domain, vector, text, lease];
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
  namespace = createSharedNamespace({registryEntries: 65536});
  const instance = await createAuthorizedHost(namespace).instantiate(bytes, grant);
  // Reentry: the sequence continues; one account is never reset or refunded,
  // so three invocations consume exactly three debits and a fourth refuses.
  const account = createAccount(namespace, 3 * grant.accountBytes);
  let previous = 0n;
  for (let run = 0; run < 3; run++) {
    const o = success(instance.invoke({account}));
    assert.equal(o.events, 1);
    assert.equal(o.cumulative, 12288);
    assert(o.firstSequence > previous);
    previous = o.nextSequence - 1n;
  }
  assert.throws(() => instance.invoke({account}), isRefusal('account'));
} else if (mode === 'controls') {
  assert.deepEqual(grant.order, ['scalar', 'text', 'payload']);
  // Room for exactly two concurrent domains' precharge, plus one entry.
  namespace = createSharedNamespace({registryEntries: 2 * grant.registryEntries + 1});
  assert.deepEqual(Reflect.ownKeys(namespace), ['observe', 'createAccount', 'createHost']);
  const opaqueAccount = exact();
  assert.deepEqual(Reflect.ownKeys(opaqueAccount), ['byteLength']);
  assert(Object.isFrozen(namespace) && Object.isFrozen(opaqueAccount));
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

  // Physical account: exact and +1 succeed; an exact account is consumed and
  // never reset; one short refuses at the first Bytes operand, after that
  // row's String, before any payload allocation.
  const once = exact();
  success(instance.invoke({account: once}));
  assert.throws(() => instance.invoke({account: once}), isRefusal('account'));
  const plus = success(instance.invoke({account: createAccount(namespace, grant.accountBytes + 1)}));
  assert.equal(plus.accountBytes, grant.accountBytes + 1);
  assert.throws(() => instance.invoke({account: createAccount(namespace, grant.accountBytes - 1)}),
    capture('metadata'));
  assert.equal(observed.domains[0].phase, 'Abandoned');
  assert.equal(observed.domains[0].allocations, 0);
  assert.equal(observed.domains[0].metadataBytes, 0);
  assert.equal(observed.accountUsed, registryReservation + rows * grant.rowBytes);
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
  for (const cut of [0, 1, 2, rows >> 1, rows - 2, rows - 1]) {
    const result = instance.invoke({account: exact(), faults: {push: cut}}), o = result.observation;
    assert.equal(result.status, 13);
    assert.equal(o.selected, 13);
    assert.equal(result.value, undefined);
    assert.equal(o.domains[0].phase, 'Settled');
    assert.equal(o.domains[0].allocations, 1);
    assert.equal(o.domains[0].releases, 1);
    assert.equal(o.events, 1);
    assert.equal(o.cumulative, extent);
    assert.equal(o.accountUsed, grant.accountBytes);
    assert.equal(o.strings, cut + 1);
    assert.equal(o.stringDrops, cut + 1);
    assert.equal(o.liveEntries, 0);
  }
  const vec = instance.invoke({account: exact(), faults: {vec: true}});
  assert.equal(vec.status, 15);
  assert.equal(vec.observation.domains[0].allocations, 0);
  assert.equal(vec.observation.events, 0);
  assert.equal(vec.observation.accountUsed, registryReservation);
  // Physical payload failure is fail-stop: debits and charges precede it.
  assert.throws(() => instance.invoke({account: exact(), faults: {payload: true}}),
    error => { observed = error.observation; return error.message === 'selected-fused:physical-payload-fault'; });
  assert.equal(observed.domains[0].phase, 'Abandoned');
  assert.equal(observed.domains[0].allocations, 0);
  assert.equal(observed.accountUsed, grant.accountBytes);
  assert.equal(observed.events, 1);
  assert.equal(observed.liveEntries, 0);
  success(instance.invoke({account: exact()}));

  // Separately loaded module copies share the one realm namespace authority:
  // neither can create a namespace, both mint from one sequence, and a live
  // token of every kind from one copy is refused by the other with no effect.
  const copyA = await import(new URL('./host.mjs?copy=A', import.meta.url));
  const copyB = await import(new URL('./host.mjs?copy=B', import.meta.url));
  refusal(() => copyA.createSharedNamespace(), 'namespace-singleton');
  refusal(() => copyB.createSharedNamespace({registryEntries: 1}), 'namespace-singleton');
  const fromA = await copyA.createAuthorizedHost(namespace).instantiate(bytes, grant, {controls: true});
  const fromB = await copyB.createAuthorizedHost(namespace).instantiate(bytes, grant, {controls: true});
  const third = await host.instantiate(bytes, grant, {controls: true});
  const outer = fromA.session({account: copyA.createAccount(namespace, grant.accountBytes)}, a => {
    let [da, va, ta, oa] = build(a), row;
    const view = a.payload_borrow(da, va, 0n);
    a.end_borrow(da, view);
    [row, va] = a.vec_pop(da, va);
    const clone = a.deep_clone(da, row);
    const borrowed = a.row_borrow(da, row);
    const inner = fromB.session({account: copyB.createAccount(namespace, grant.accountBytes)}, b => {
      const db = b.start(R, W);
      // Two domains' precharge are held; a third cannot be precharged.
      assert.throws(() => third.session({account: exact()}, c => c.start(R, W)), isRefusal('registry-exhausted'));
      const vb = b.vec_new(db, R), tb = b.string_literal(db, LITERAL, textBytes), ob = b.issue(db, 0n);
      const tokensA = [da, va, ta, oa, borrowed, row, clone], tokensB = [db, vb, tb, ob];
      for (const left of tokensA) for (const right of tokensB) assert.notEqual(left, right);
      refusal(() => b.vec_new(da, R), 'foreign-host');
      b.abort(db);
      for (const [operation, token] of [[b.vec_drop, va], [b.string_drop, ta], [b.owner_drop, oa],
        [b.end_borrow, borrowed], [b.row_drop, row], [b.bytes_drop, clone]]) {
        refusal(() => operation(db, token), 'foreign-host');
      }
      refusal(() => b.owner_drop(db, ob + (1n << 20n)), 'stale');
      b.owner_drop(db, ob);
      b.string_drop(db, tb);
      b.vec_drop(db, vb);
      b.settle_abort(db);
      return 0n;
    });
    assert.equal(inner.observation.domains[0].phase, 'Settled');
    assert.equal(inner.observation.domains[0].failure, 'foreign-host');
    // No effect on the owner: every presented token still authenticates.
    assert.equal(a.view_byte(da, borrowed, 0n), 0);
    a.end_borrow(da, borrowed);
    assert.equal(a.bytes_byte(da, clone, 0), 0);
    a.bytes_drop(da, clone);
    a.row_drop(da, row);
    a.vec_drop(da, va);
    return 0n;
  });
  assert.equal(outer.observation.domains[0].phase, 'Settled');
  assert.equal(outer.observation.domains[0].failure, null);
  assert.equal(outer.observation.domains[0].releases, 1);
  assert.equal(outer.observation.liveEntries, 0);
  assert.deepEqual(outer.observation.finalizers.filter(event => event.container === 'row').map(event => event.kind),
    [...grant.order].reverse().filter(role => role !== 'scalar'));
  // Foreign instance of the same host copy.
  const sibling = await host.instantiate(bytes, grant, {controls: true});
  instance.session({account: exact()}, a => {
    const da = a.start(R, W), va = a.vec_new(da, R);
    sibling.session({account: exact()}, b => {
      const db = b.start(R, W);
      refusal(() => b.vec_new(da, R), 'foreign-instance');
      refusal(() => b.vec_len(db, va), 'foreign-instance');
      b.abort(db);
      b.settle_abort(db);
      return 0n;
    });
    a.abort(da);
    a.vec_drop(da, va);
    a.settle_abort(da);
    return 0n;
  });
  // Foreign domain within one instance and invocation.
  instance.session({account: createAccount(namespace, 2 * grant.accountBytes)}, c => {
    const first = c.start(R, W);
    c.abort(first);
    c.settle_abort(first);
    const second = c.start(R, W), vector = c.vec_new(second, R);
    refusal(() => c.vec_len(second, first), 'foreign');
    c.abort(second);
    c.vec_drop(second, vector);
    c.settle_abort(second);
    return 0n;
  });
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
    for (let slot = 0; slot < 64; slot++) open.push(c.payload_borrow(domain, vector, BigInt(slot)));
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
  // Construction order and phase refusals. Token authentication precedes the
  // phase check: a stale operand in Building is stale, not a phase refusal.
  settledControl(instance, c => {
    const d = c.start(R, W), v = c.vec_new(d, R);
    refusal(() => c.vec_len(d, v + (1n << 20n)), 'stale');
    c.abort(d); c.vec_drop(d, v); c.settle_abort(d);
    return 0n;
  }, 'stale');
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
  // Sequence exhaustion refuses before mutation, in every module copy, and
  // the shared namespace never resets.
  namespace = createSharedNamespace({registryEntries: grant.registryEntries, sequenceLimit: 3});
  const exhausted = await createAuthorizedHost(namespace).instantiate(bytes, grant, {controls: true});
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
  const copy = await import(new URL('./host.mjs?copy=exhaustion', import.meta.url));
  refusal(() => copy.createSharedNamespace(), 'namespace-singleton');
  const later = await copy.createAuthorizedHost(namespace).instantiate(bytes, grant);
  assert.throws(() => later.invoke({account: copy.createAccount(namespace, grant.accountBytes)}),
    isRefusal('namespace-exhausted'));
  assert.equal(namespace.observe().nextSequence, 4n);
} else {
  throw new Error(`unknown selected driver mode ${mode}`);
}
console.log('selected-fused-wasm-v1-ok');
