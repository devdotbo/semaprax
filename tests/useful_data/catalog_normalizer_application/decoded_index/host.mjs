import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const module = new WebAssembly.Module(readFileSync(process.argv[2]));
const provider = environmentProvider(module, 69710, true);
assert.equal(provider.acceptsOwnedLength(69710), true);
assert.equal(provider.acceptsOwnedLength(69711), false);
assert.equal(provider.acceptsOwnedLength(Number.MAX_SAFE_INTEGER + 1), false);
assert.throws(() => provider.imports.env.spx_bytes_zeroed(69711n), /arena capacity/);
assert.throws(() => provider.imports.env.spx_bytes_zeroed((1n << 63n) - 1n), /arena capacity/);
assert.equal(provider.live(), 0);
assert.deepEqual(
  WebAssembly.Module.imports(module).map(item => item.name).sort(),
  ['spx_add', 'spx_sub', 'spx_mul', 'spx_div', 'spx_rem', 'spx_neg',
    'spx_contract_fail', 'spx_bytes_copy', 'spx_bytes_get', 'spx_bytes_drop',
    'spx_bytes_as_slice', 'spx_bytes_zeroed', 'spx_bytes_set',
    'spx_bytes_set5', 'spx_bytes_set1_or5', 'spx_bytes_set1_or6_or48',
    'spx_vec_with_capacity', 'spx_vec_push', 'spx_vec_len',
    'spx_vec_capacity', 'spx_vec_get', 'spx_vec_drop',
    'spx_vec_reserve_exact', 'spx_vec_set', 'spx_vec_clear'].sort(),
);
const min = -(1n << 63n);
const max = (1n << 63n) - 1n;
const checked = operation => (...args) => {
  const value = operation(...args);
  if (value < min || value > max) throw new RangeError('checked i64 overflow');
  return value;
};
const div = (left, right) => {
  if (right === 0n || (left === min && right === -1n)) throw new RangeError('checked i64 division');
  return left / right;
};
const rem = (left, right) => {
  if (right === 0n || (left === min && right === -1n)) throw new RangeError('checked i64 remainder');
  return left % right;
};
Object.assign(provider.imports.env, {
  spx_add: checked((left, right) => left + right),
  spx_sub: checked((left, right) => left - right),
  spx_mul: checked((left, right) => left * right),
  spx_neg: checked(value => -value),
  spx_div: div,
  spx_rem: rem,
});
const incomplete = { env: { ...provider.imports.env } };
delete incomplete.env.spx_bytes_set;
assert.throws(() => new WebAssembly.Instance(module, incomplete), WebAssembly.LinkError);
// Each newly declared import remains mandatory, rather than an ignored stub.
for (const name of ['spx_vec_reserve_exact','spx_vec_set','spx_vec_clear']) {
  const missing = {env:{...provider.imports.env}};delete missing.env[name];
  assert.throws(()=>new WebAssembly.Instance(module,missing),WebAssembly.LinkError);
}
const env=provider.imports.env;
let owner=env.spx_vec_with_capacity(1,2n);
owner=env.spx_vec_push(owner,1,7n);
assert.equal(env.spx_vec_reserve_exact(owner,1,-1n),0n);
assert.equal(env.spx_vec_reserve_exact(owner,1,8192n),0n);
assert.equal(env.spx_vec_len(owner,1),1n);
const beforeReserve=owner;owner=env.spx_vec_reserve_exact(owner,1,3n);
assert.equal(env.spx_vec_capacity(owner,1),4n);
assert.throws(()=>env.spx_vec_len(beforeReserve,1),/stale/);
assert.throws(()=>env.spx_vec_set(owner,3,0n,9n),/mistyped/);
assert.equal(env.spx_vec_set(owner,1,-1n,9n),0n);
assert.equal(env.spx_vec_set(owner,1,1n,9n),0n);
assert.equal(env.spx_vec_get(owner,1,0n),7n);
const beforeSet=owner;owner=env.spx_vec_set(owner,1,0n,9n);
assert.equal(env.spx_vec_get(owner,1,0n),9n);
assert.throws(()=>env.spx_vec_len(beforeSet,1),/stale/);
const beforeClear=owner;owner=env.spx_vec_clear(owner,1);
assert.equal(env.spx_vec_len(owner,1),0n);
assert.equal(env.spx_vec_capacity(owner,1),4n);
assert.throws(()=>env.spx_vec_len(beforeClear,1),/stale/);
env.spx_vec_drop(owner);assert.throws(()=>env.spx_vec_clear(owner,1),/stale/);
assert.equal(provider.live(),0);
const instance = new WebAssembly.Instance(module, provider.imports);
provider.attach(instance);
const maximal = provider.imports.env.spx_bytes_zeroed(69710n);
assert.equal(provider.live(), 1);
provider.imports.env.spx_bytes_drop(maximal);
assert.equal(provider.live(), 0);
for (let run = 0; run < 2; run += 1) {
  assert.equal(instance.exports.semaprax_main(), 0n);
  provider.settled();
}
