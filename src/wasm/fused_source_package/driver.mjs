// Owning driver for a PUBLISHED fused-construction-v1 Core-Wasm package.
// Authored; executed by root after integration. It imports the package's own
// entry (which loads only its compiler-paired module through the fixed
// loader) and, for the language-failure control only, the shipped host.
import path from 'node:path';
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';

const [directory, mode, expectedText, sourceSha256] = process.argv.slice(2);
const entry = await import(pathToFileURL(path.join(directory, 'fused-construction-v1.mjs')).href);
const {MANIFEST} = entry;
const REALM_KEY = Symbol.for('semaprax.private.selected-fused.realm-namespace.v1');
const registry = MANIFEST.funding.registry_buffer_bytes;
const perInvocation = MANIFEST.funding.per_invocation_arena_bytes;
const limit = MANIFEST.funding.account_bytes;

assert(Object.isFrozen(MANIFEST) && Object.isFrozen(MANIFEST.grant) && Object.isFrozen(MANIFEST.funding));
assert.equal(MANIFEST.profile, 'fused-construction-v1');
assert.equal(MANIFEST.strategy, 'FusedConstructionV1');
assert.equal(MANIFEST.target, 'wasm');
assert.equal(MANIFEST.result_schema, 'semaprax.fused-source.v1');
assert.equal(MANIFEST.source_sha256, sourceSha256);
assert.equal(MANIFEST.grant.digest, MANIFEST.module_sha256);
assert.equal(MANIFEST.funding.fuel, 'none');
assert.equal(MANIFEST.grant.accountBytes, perInvocation);
assert.equal(registry, MANIFEST.grant.registryEntries * (8 + 4 * 6 + 1));

const KEYS = ['schema', 'profile', 'source_sha256', 'target', 'account_limit', 'managed_reserved', 'outcome'];
function bound(envelope, outcome, accountLimit, managedReserved) {
  assert(Object.isFrozen(envelope));
  const payload = {returned: 'result', language_failure: 'status', administrative_refusal: 'reason'}[outcome];
  assert.deepEqual(Object.keys(envelope).sort(), [...KEYS, payload].sort());
  assert.equal(envelope.schema, 'semaprax.fused-source.v1');
  assert.equal(envelope.profile, 'fused-construction-v1');
  assert.equal(envelope.source_sha256, sourceSha256);
  assert.equal(envelope.target, 'wasm');
  assert.equal(envelope.account_limit, accountLimit);
  assert.equal(envelope.managed_reserved, managedReserved);
  assert.equal(envelope.outcome, outcome);
  return envelope;
}

if (mode === 'exact') {
  // Compiled for the registry plus exactly three invocations' arena.
  assert.equal(limit, registry + 3 * perInvocation);
  // Argument and funding refusals have no effect and consume no attempt.
  for (const amount of [0, -1, 1.5, Number.NaN, '1', limit + 1, 1073741825, registry - 1]) {
    await assert.rejects(entry.install(amount), /semaprax-fused-package:account/);
  }
  assert.equal(Object.prototype.hasOwnProperty.call(globalThis, REALM_KEY), false);
  const first = entry.install(limit);
  await assert.rejects(entry.install(limit), /semaprax-fused-package:install-attempted/);
  const runner = await first;
  for (let run = 1; run <= 3; run++) {
    const returned = bound(runner.invoke(), 'returned', limit, registry + run * perInvocation);
    assert.equal(returned.result, expectedText);
  }
  // No reset, no refund: the fourth invocation is administrative and the
  // cumulative reservation is unchanged.
  assert.equal(bound(runner.invoke(), 'administrative_refusal', limit, limit).reason, 'account');
  await assert.rejects(entry.install(limit), /semaprax-fused-package:install-attempted/);
} else if (mode === 'one-short') {
  assert.equal(limit, registry + perInvocation - 1);
  const runner = await entry.install(limit);
  // The arena remainder refuses at the first Bytes operand's tables, after
  // the registry debit and row backing it actually charged...
  const tables = Math.ceil(MANIFEST.grant.metadataBytes / 8) * 8;
  const charged = registry + perInvocation - tables;
  assert.equal(bound(runner.invoke(), 'administrative_refusal', limit, charged).reason, 'metadata');
  // ...and nothing is refunded, so a retry cannot fit.
  assert.equal(bound(runner.invoke(), 'administrative_refusal', limit, charged).reason, 'account');
} else if (mode === 'tampered') {
  // The Rust gate tampered with the published paired module file; the public
  // loader refuses it before the realm namespace exists.
  await assert.rejects(entry.install(limit), /semaprax-fused-package:module-digest/);
  assert.equal(Object.prototype.hasOwnProperty.call(globalThis, REALM_KEY), false);
  await assert.rejects(entry.install(limit), /semaprax-fused-package:install-attempted/);
} else if (mode === 'language') {
  // Language failure is injected through the shipped host's fault control on
  // the same sealed grant and funding split; normalization is the entry's own.
  assert.equal(limit, registry + 3 * perInvocation);
  const fs = await import('node:fs');
  const host = await import(pathToFileURL(path.join(directory, 'selected-host.mjs')).href);
  const moduleBytes = fs.readFileSync(path.join(directory, 'fused-construction-v1.wasm'));
  const namespace = host.createSharedNamespace({registryEntries: MANIFEST.grant.registryEntries});
  const instance = await host.createAuthorizedHost(namespace).instantiate(moduleBytes, MANIFEST.grant);
  const account = host.createAccount(namespace, limit - registry);
  let used = 0;
  const reserved = observation => registry + (used += observation?.accountUsed ?? 0);
  const step = run => {
    const raw = instance.invoke(run);
    return entry.normalize(raw, limit, reserved(raw.observation));
  };
  // An injected push refusal takes the push's own failure exit: push_full.
  const failed = bound(step({account, faults: {push: 7}}), 'language_failure', limit, registry + perInvocation);
  assert.deepEqual(failed.status, {domain: 'semaprax.vec.v1', code: 1});
  assert.equal(bound(step({account}), 'returned', limit, registry + 2 * perInvocation).result, expectedText);
  const again = bound(step({account, faults: {push: 4095}}), 'language_failure', limit, limit);
  assert.deepEqual(again.status, {domain: 'semaprax.vec.v1', code: 1});
  // The pure normalizer keeps push-full, read and allocation codes distinct;
  // these raw records are direct helper inputs, not source witnesses.
  for (const [raw, code] of [[13, 1], [14, 2], [15, 3]]) {
    const normalized = bound(entry.normalize({status: raw}, limit, limit), 'language_failure', limit, limit);
    assert.deepEqual(normalized.status, {domain: 'semaprax.vec.v1', code});
  }
  assert.throws(() => instance.invoke({account}), error => {
    const refused = entry.administrative(error.code, limit, reserved(error.observation));
    assert.equal(bound(refused, 'administrative_refusal', limit, limit).reason, 'account');
    return true;
  });
} else {
  throw new Error(`unknown fused package driver mode ${mode}`);
}
console.log('fused-source-package-v1-ok');
