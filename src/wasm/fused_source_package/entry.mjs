// Generated `fused-construction-v1` Core-Wasm entry for one sealed selection.
// Trusted first-install realm glue: it is not a JavaScript sandbox and it
// claims no total JavaScript-heap or process (RSS) bound. No fuel is enforced
// on this target. The manifest below is emitted with the paired module from
// the same validated owner; no caller can supply a module, grant or path.
//
// Funding. `install(accountBytes)` takes the caller's explicit finite limit.
// That one limit funds both eagerly allocated physical buffers: the realm
// namespace registry (reserved first, exactly registryEntries * 33 bytes) and
// the invocation account arena (the remainder, allocated eagerly in full).
// The arena carries the host's unchanged per-invocation debits: registry
// precharge bytes, row backing and lease/view tables. JavaScript objects,
// Maps, closures, String bytes and the ordinary Bytes payload are not funded
// by this limit; the payload stays under the ordinary 32-event/2 MiB counters.
import {createAccount, createAuthorizedHost, createSharedNamespace} from './selected-host.mjs';

const deepFreeze = value => {
  if (value !== null && typeof value === 'object') {
    for (const field of Object.values(value)) deepFreeze(field);
    Object.freeze(value);
  }
  return value;
};
export const MANIFEST = deepFreeze(__SEMAPRAX_FUSED_MANIFEST__);
const MAX_ACCOUNT_LIMIT = 1073741824;
const MODULE_URL = new URL('./fused-construction-v1.wasm', import.meta.url);
// Raw selected status -> normalized (domain, code). Only statuses the
// selected lowering can select are mapped; anything else is administrative.
const STATUS = Object.freeze({
  1: Object.freeze({domain: 'semaprax.arithmetic.v1', code: 1}),
  2: Object.freeze({domain: 'semaprax.arithmetic.v1', code: 2}),
  13: Object.freeze({domain: 'semaprax.vec.v1', code: 1}),
  14: Object.freeze({domain: 'semaprax.vec.v1', code: 2}),
  15: Object.freeze({domain: 'semaprax.vec.v1', code: 3}),
  21: Object.freeze({domain: 'semaprax.convert.v1', code: 1}),
});

async function sha256Hex(bytes) {
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256', bytes));
  return Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}
// Fixed loader for the compiler-paired module only.
async function loadPairedModule() {
  if (typeof globalThis.process?.versions?.node === 'string') {
    const {readFile} = await import('node:fs/promises');
    return new Uint8Array(await readFile(MODULE_URL));
  }
  const response = await fetch(MODULE_URL);
  if (!response.ok) throw new Error('semaprax-fused-package:module-unavailable');
  return new Uint8Array(await response.arrayBuffer());
}

// Shared `semaprax.fused-source.v1` envelope. `result` is published only on
// `returned`; `language_failure` carries a normalized status and no result;
// `administrative_refusal` carries a reason and neither result nor status.
function envelope(accountLimit, managedReserved, outcome, fields) {
  return Object.freeze({schema: 'semaprax.fused-source.v1', profile: MANIFEST.profile,
    source_sha256: MANIFEST.source_sha256, target: 'wasm', account_limit: accountLimit,
    managed_reserved: managedReserved, outcome, ...fields});
}
// Pure envelope helpers; they hold no authority.
export function normalize(raw, accountLimit, managedReserved) {
  if (raw.status === 0) {
    return envelope(accountLimit, managedReserved, 'returned',
      {result: BigInt.asIntN(64, raw.value).toString()});
  }
  const status = STATUS[raw.status];
  return status === undefined
    ? administrative('unmapped-status', accountLimit, managedReserved)
    : envelope(accountLimit, managedReserved, 'language_failure', {status});
}
export function administrative(reason, accountLimit, managedReserved) {
  return envelope(accountLimit, managedReserved, 'administrative_refusal', {reason: String(reason)});
}

let attempted = false;
// One install per realm. Argument and funding refusals have no effect; once
// the arguments are accepted, every later or concurrent attempt is denied.
export async function install(accountBytes) {
  if (!Number.isSafeInteger(accountBytes) || accountBytes < 1 || accountBytes > MAX_ACCOUNT_LIMIT
      || accountBytes > MANIFEST.funding.account_bytes) throw new TypeError('semaprax-fused-package:account-limit');
  const registryBytes = MANIFEST.funding.registry_buffer_bytes;
  if (accountBytes < registryBytes) throw new Error('semaprax-fused-package:account');
  if (attempted) throw new Error('semaprax-fused-package:install-attempted');
  attempted = true;
  const bytes = await loadPairedModule();
  // The digest is checked before the realm namespace exists.
  if (await sha256Hex(bytes) !== MANIFEST.module_sha256) throw new Error('semaprax-fused-package:module-digest');
  const namespace = createSharedNamespace({registryEntries: MANIFEST.grant.registryEntries});
  const account = createAccount(namespace, accountBytes - registryBytes);
  const instance = await createAuthorizedHost(namespace).instantiate(bytes, MANIFEST.grant);
  // Cumulative charges across every invocation; never reset or refunded.
  let arenaUsed = 0, poisoned = false;
  const charge = observation => { arenaUsed += observation?.accountUsed ?? 0; return registryBytes + arenaUsed; };
  return Object.freeze({
    invoke() {
      if (poisoned) throw new Error('semaprax-fused-package:fail-stop');
      let raw;
      try {
        raw = instance.invoke({account});
      } catch (error) {
        if (error?.name === 'SelectedRefusal') {
          return administrative(error.code, accountBytes, charge(error.observation));
        }
        poisoned = true;
        throw error;
      }
      return normalize(raw, accountBytes, charge(raw.observation));
    },
  });
}
