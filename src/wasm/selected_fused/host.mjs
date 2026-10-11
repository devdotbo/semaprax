// Selected FusedConstructionV1 Core-Wasm host for the private namespace
// `semaprax.private.selected-fused.v1`. It is not a public Web adapter, a
// frozen Bytes ABI or a default import; it never serves an `env` module.
//
// Handle authority. A realm has exactly one caller-held shared namespace
// (`createSharedNamespace`; a second call refuses). Every authorized host must
// be created with that namespace, so two hosts can never both start counting
// at one. The namespace owns a never-reset, nonwrapping sequence and a
// preallocated typed registry: each token is (sequence << 20) | entry, and the
// entry records kind, host, instance, invocation and owner domain. Every
// operation resolves the exact token and checks all five bindings before any
// read or mutation, so an equal number from another host, instance,
// invocation or domain is refused with no effect.
//
// Physical accounting. The registry is one preallocated ArrayBuffer; each
// domain precharges its maximum live entries at `start`. Row backing and the
// lease/view tables are carved from one caller-held, preallocated invocation
// account ArrayBuffer before the payload is allocated; carving checks the
// whole group first. JavaScript Map, array, object and closure bookkeeping
// (holder objects, Strings' ordinary byte arrays) is NOT claimed as a
// physical reservation. Payload and String bytes are ordinary allocations
// charged to the ordinary 32-event/2 MiB and String counters.

export const NAMESPACE = 'semaprax.private.selected-fused.v1';
export const IMPORT_NAMES = Object.freeze([
  'start', 'vec_new', 'string_literal', 'issue', 'row_push', 'complete', 'abort',
  'settle_abort', 'vec_move', 'vec_len', 'field_scalar', 'field_text', 'text_len',
  'text_byte', 'payload_borrow', 'view_len', 'view_byte', 'end_borrow', 'sort',
  'vec_drop', 'string_drop', 'owner_drop', 'select_failure', 'return_failure',
]);
const MAX_VIEWS = 64;
const MAX_EVENTS = 32;
const MAX_CUMULATIVE = 2097152;
const MAX_EXTENT = 131072;
const MIN_ROWS = 33;
const MAX_ROWS = 4096;
const SLOT_BYTES = 8 + 8 + 1 + 1 + 4;
const VIEW_BYTES = 8 + 4 + 8 + 1;
const ROW_BYTES = 8 + 8 + 8;
const REGISTRY_ENTRY_BYTES = 8 + 4 * 6 + 1;
const MAX_CLONES = MAX_EVENTS - 1;
const VEC_MAX_CAPACITY = 8192;
const VEC_MAX_OWNED_BYTES = 131072;
const OWNED_BYTES_PER_LEAF = 16;
const LITERAL_BASE = 1024;
const INDEX_BITS = 20n;
const INDEX_MASK = (1n << INDEX_BITS) - 1n;
const MAX_ENTRIES = 1 << 20;
const SEQUENCE_LIMIT = (1n << 43n) - 1n;
const KIND = Object.freeze({domain: 1, vec: 2, string: 3, owner: 4, view: 5, row: 6, bytes: 7});

export class SelectedRefusal extends Error {
  constructor(code) { super(`selected-fused:${code}`); this.code = code; }
}
export class SelectedStatus extends Error {
  constructor(status) { super(`status:${status}`); this.status = status; }
}
const refuse = code => { throw new SelectedRefusal(code); };

function validUtf8(bytes) {
  for (let index = 0; index < bytes.length;) {
    const first = bytes[index++];
    let extra, minimum, scalar;
    if (first < 128) continue;
    if (first >= 194 && first <= 223) { extra = 1; minimum = 128; scalar = first & 31; }
    else if (first >= 224 && first <= 239) { extra = 2; minimum = 2048; scalar = first & 15; }
    else if (first >= 240 && first <= 244) { extra = 3; minimum = 65536; scalar = first & 7; }
    else return false;
    if (index + extra > bytes.length) return false;
    for (let count = 0; count < extra; count++) {
      const byte = bytes[index++];
      if ((byte & 192) !== 128) return false;
      scalar = (scalar << 6) | (byte & 63);
    }
    if (scalar < minimum || scalar > 1114111 || (scalar >= 55296 && scalar <= 57343)) return false;
  }
  return true;
}
function compareBytes(left, right) {
  const bound = Math.min(left.length, right.length);
  for (let index = 0; index < bound; index++) if (left[index] !== right[index]) return left[index] - right[index];
  return left.length - right.length;
}
async function sha256(bytes) {
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest('SHA-256', bytes));
  return Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}
// Typed views carved in order from one preallocated buffer; the caller has
// already checked the group fits. 8-byte arrays precede 4- and 1-byte ones.
function carveAll(buffer, start, layout) {
  let offset = start;
  const views = {};
  for (const [name, Type, count] of layout) {
    if (offset % Type.BYTES_PER_ELEMENT !== 0) throw new Error('selected carving alignment invariant');
    views[name] = new Type(buffer, offset, count);
    views[name].fill(Type === BigUint64Array || Type === BigInt64Array ? 0n : 0);
    offset += Type.BYTES_PER_ELEMENT * count;
  }
  return {views, end: offset};
}
const layoutBytes = layout => layout.reduce((sum, [, Type, count]) => sum + Type.BYTES_PER_ELEMENT * count, 0);
const rowLayout = rows => [['scalar', BigInt64Array, rows], ['text', BigUint64Array, rows],
  ['owner', BigUint64Array, rows]];
const tableLayout = rows => [['nonce', BigUint64Array, rows], ['generation', BigUint64Array, rows],
  ['viewNonce', BigUint64Array, MAX_VIEWS], ['viewGeneration', BigUint64Array, MAX_VIEWS],
  ['borrows', Uint32Array, rows], ['viewSlot', Uint32Array, MAX_VIEWS], ['live', Uint8Array, rows],
  ['published', Uint8Array, rows], ['viewLive', Uint8Array, MAX_VIEWS]];
const registryEntries = rows => 2 + 3 * rows + MAX_VIEWS + MAX_CLONES + 1;

// ---- The realm's single caller-held namespace -------------------------------
let realm = null;
const namespaces = new WeakMap();
const accounts = new WeakMap();

export function createSharedNamespace(options = {}) {
  if (realm !== null) refuse('namespace-singleton');
  const entries = options.registryEntries ?? 65536;
  const sequenceLimit = options.sequenceLimit === undefined ? SEQUENCE_LIMIT : BigInt(options.sequenceLimit);
  if (!Number.isSafeInteger(entries) || entries < 1 || entries > MAX_ENTRIES
      || sequenceLimit < 1n || sequenceLimit > SEQUENCE_LIMIT) throw new TypeError('selected namespace dimensions');
  const buffer = new ArrayBuffer(entries * REGISTRY_ENTRY_BYTES);
  const {views: registry, end} = carveAll(buffer, 0, [['token', BigUint64Array, entries],
    ['host', Uint32Array, entries], ['instance', Uint32Array, entries], ['invocation', Uint32Array, entries],
    ['domain', Uint32Array, entries], ['target', Uint32Array, entries], ['free', Uint32Array, entries],
    ['kind', Uint8Array, entries]]);
  if (end !== buffer.byteLength) throw new Error('selected registry layout invariant');
  for (let index = 0; index < entries; index++) registry.free[index] = entries - 1 - index;
  const state = {registry, entries, objects: new Array(entries).fill(null), sequence: 1n, sequenceLimit,
    freeTop: entries, quota: 0, hosts: 0, instances: 0, invocations: 0, registryBytes: buffer.byteLength};
  const handle = Object.freeze({
    observe: () => Object.freeze({nextSequence: state.sequence, liveEntries: entries - state.freeTop,
      outstandingQuota: state.quota, registryBytes: state.registryBytes}),
  });
  namespaces.set(handle, state);
  realm = handle;
  return handle;
}

// One caller-held, preallocated invocation account.
export function createAccount(bytes) {
  if (!Number.isSafeInteger(bytes) || bytes < 0) throw new TypeError('selected account size');
  const handle = Object.freeze({byteLength: bytes});
  accounts.set(handle, {buffer: new ArrayBuffer(bytes), offset: 0, held: null});
  return handle;
}

function checkGrant(grant) {
  const {rows, width, metadataBytes, rowBytes, accountBytes, order} = grant;
  const roles = ['scalar', 'text', 'payload'];
  if (!Number.isSafeInteger(rows) || rows < MIN_ROWS || rows > MAX_ROWS
      || !Number.isSafeInteger(width) || width < 1 || rows * width > MAX_EXTENT
      || rows > VEC_MAX_CAPACITY || rows * 2 * OWNED_BYTES_PER_LEAF > VEC_MAX_OWNED_BYTES
      || metadataBytes !== layoutBytes(tableLayout(rows)) || rowBytes !== ROW_BYTES
      || accountBytes !== layoutBytes(rowLayout(rows)) + metadataBytes
      || grant.registryEntries !== registryEntries(rows)
      || !Array.isArray(order) || order.length !== 3 || roles.some(role => !order.includes(role))
      || typeof grant.digest !== 'string') refuse('grant');
  return Object.freeze({rows, width, metadataBytes, accountBytes, registryEntries: grant.registryEntries,
    order: Object.freeze([...order]), textFirst: order.indexOf('text') < order.indexOf('payload')});
}

export function createAuthorizedHost(namespace) {
  if (realm === null || namespace !== realm) refuse('namespace');
  const space = namespaces.get(namespace);
  const registry = space.registry;
  const host = ++space.hosts;

  // Resolve an exact live token to its registry entry, or null.
  const resolve = carrier => {
    if (typeof carrier !== 'bigint') refuse('carrier');
    const token = BigInt.asUintN(64, carrier);
    const index = Number(token & INDEX_MASK);
    if (token === 0n || index >= space.entries || registry.token[index] !== token) return null;
    return {token, index};
  };
  // Mint from a domain's precharge; refuses before any mutation.
  const reserve = (domain, count) => {
    if (domain.quota < count) refuse('registry-quota');
    if (space.sequence + BigInt(count) - 1n > space.sequenceLimit) refuse('namespace-exhausted');
  };
  const mint = (domain, kind, target, object, binding) => {
    reserve(domain, 1);
    const index = registry.free[--space.freeTop];
    const token = (space.sequence << INDEX_BITS) | BigInt(index);
    space.sequence += 1n;
    registry.token[index] = token;
    registry.kind[index] = kind;
    registry.host[index] = host;
    registry.instance[index] = binding.instance;
    registry.invocation[index] = binding.invocation;
    registry.domain[index] = kind === KIND.domain ? index : domain.index;
    registry.target[index] = target;
    space.objects[index] = object;
    domain.quota--;
    space.quota--;
    return {token, index};
  };
  // Retired entries return to the owning domain's precharge, never to others.
  const retire = (domain, token) => {
    const found = resolve(token);
    if (found === null) refuse('stale');
    registry.token[found.index] = 0n;
    space.objects[found.index] = null;
    registry.free[space.freeTop++] = found.index;
    domain.quota++;
    space.quota++;
  };
  const releaseQuota = domain => { space.quota -= domain.quota; domain.quota = 0; };

  async function instantiate(bytes, rawGrant, setup = {}) {
    const copy = new Uint8Array(bytes);
    const grant = checkGrant(rawGrant);
    if (await sha256(copy) !== rawGrant.digest) refuse('module-digest');
    const module = await WebAssembly.compile(copy);
    const imports = WebAssembly.Module.imports(module);
    // Exactly the private inventory: no public `env` import can be mixed in.
    if (imports.length !== IMPORT_NAMES.length || imports.some((entry, index) =>
      entry.module !== NAMESPACE || entry.name !== IMPORT_NAMES[index] || entry.kind !== 'function'))
      refuse('import-inventory');
    const exported = WebAssembly.Module.exports(module).map(entry => `${entry.name}:${entry.kind}`).sort();
    if (exported.join(',') !== 'memory:memory,semaprax_main:function') refuse('export-inventory');
    const instance = ++space.instances;
    let invocation = null, memory = null;

    const binding = () => ({instance, invocation: invocation.serial});
    const domainOf = carrier => {
      const found = resolve(carrier);
      if (found === null) refuse('stale');
      const index = found.index;
      if (registry.kind[index] !== KIND.domain) refuse('kind');
      if (registry.host[index] !== host) refuse('foreign-host');
      if (registry.instance[index] !== instance) refuse('foreign-instance');
      if (invocation === null || registry.invocation[index] !== invocation.serial) refuse('foreign-invocation');
      return space.objects[index];
    };
    const fault = (domain, code) => {
      if (domain.failure === null) domain.failure = code;
      refuse(code);
    };
    const held = (domain, carrier, kind) => {
      const found = resolve(carrier);
      if (found === null) fault(domain, 'stale');
      const index = found.index;
      if (registry.host[index] !== host) fault(domain, 'foreign-host');
      if (registry.instance[index] !== instance) fault(domain, 'foreign-instance');
      if (registry.invocation[index] !== invocation.serial) fault(domain, 'foreign-invocation');
      if (registry.domain[index] !== domain.index) fault(domain, 'foreign');
      if (registry.kind[index] !== kind) fault(domain, 'kind');
      return {handle: found.token, entry: space.objects[index], target: registry.target[index]};
    };
    const active = domain => { if (domain.failure !== null) refuse(domain.failure); };
    const phase = (domain, ...allowed) => { if (!allowed.includes(domain.phase)) fault(domain, 'phase'); };
    const index = (domain, vec, position) => {
      if (typeof position !== 'bigint' || position < 0n || position >= BigInt(vec.len)) fault(domain, 'range');
      return Number(position);
    };
    const owner = (domain, carrier) => {
      const {handle, entry, target: slot} = held(domain, carrier, KIND.owner);
      const tables = domain.tables;
      if (!tables.live[slot] || tables.nonce[slot] !== handle) fault(domain, 'stale');
      return {handle, entry, slot};
    };
    const view = (domain, carrier) => {
      const {handle, target: at} = held(domain, carrier, KIND.view);
      const tables = domain.tables, slot = tables.viewSlot[at];
      if (!tables.viewLive[at] || tables.viewNonce[at] !== handle || !tables.live[slot]
          || tables.generation[slot] !== tables.viewGeneration[at] || tables.borrows[slot] === 0)
        fault(domain, 'stale');
      return {handle, at, slot};
    };
    const releaseIfLast = domain => {
      if (domain.children !== 0 || domain.pin) return;
      if (domain.payload !== null) {
        domain.payload = null;
        domain.releases++;
      }
      if (domain.vec === null && domain.strings === 0 && domain.views === 0 && domain.clones === 0)
        domain.phase = 'Settled';
    };
    const dropString = (domain, carrier, container) => {
      const {handle, entry} = held(domain, carrier, KIND.string);
      if (entry.contained !== container) fault(domain, 'contained');
      retire(domain, handle);
      domain.strings--;
      invocation.ledger.stringDrops++;
    };
    const dropOwner = (domain, carrier, container) => {
      const {handle, entry, slot} = owner(domain, carrier);
      if (entry.contained !== container) fault(domain, 'contained');
      if (domain.tables.borrows[slot] !== 0) fault(domain, 'borrowed');
      domain.tables.live[slot] = 0;
      retire(domain, handle);
      domain.children--;
    };
    // A renewed token is minted before the old one retires (one overlap entry).
    const renew = (domain, kind, target, object, old) => {
      reserve(domain, 1);
      const fresh = mint(domain, kind, target, object, binding());
      retire(domain, old);
      return fresh.token;
    };
    const borrowSlot = (domain, slot) => {
      const tables = domain.tables;
      const at = tables.viewLive.indexOf(0);
      if (at < 0) fault(domain, 'view-capacity');
      if (tables.borrows[slot] === 0xffffffff) fault(domain, 'borrow-overflow');
      reserve(domain, 1);
      const {token} = mint(domain, KIND.view, at, null, binding());
      tables.viewNonce[at] = token;
      tables.viewSlot[at] = slot;
      tables.viewGeneration[at] = tables.generation[slot];
      tables.viewLive[at] = 1;
      tables.borrows[slot]++;
      domain.views++;
      domain.peakViews = Math.max(domain.peakViews, domain.views);
      return token;
    };
    const range = (domain, slot) => domain.payload.subarray(slot * grant.width, (slot + 1) * grant.width);
    // Transactional carving from the held invocation account.
    const carve = (domain, layout) => {
      const account = invocation.account, bytes = layoutBytes(layout);
      if (account.offset + bytes > account.buffer.byteLength) fault(domain, 'metadata');
      const {views, end} = carveAll(account.buffer, account.offset, layout);
      account.offset = end;
      invocation.ledger.accountCarved = end;
      return {views, bytes};
    };

    const env = {
      start(rows, width) {
        if (invocation === null) refuse('no-invocation');
        if (rows !== BigInt(grant.rows) || width !== BigInt(grant.width)) refuse('grant-dimensions');
        if (invocation.domains.some(domain => domain.phase !== 'Settled')) refuse('domain-active');
        // Precharge every registry entry this domain can hold live.
        if (space.freeTop - space.quota < grant.registryEntries) refuse('registry-exhausted');
        if (space.sequence > space.sequenceLimit) refuse('namespace-exhausted');
        const domain = {index: 0, quota: grant.registryEntries, phase: 'Building', pin: true, failure: null,
          issued: 0, children: 0, strings: 0, views: 0, peakViews: 0, clones: 0, vec: null, tables: null,
          payload: null, staged: {text: null, owner: null}, allocations: 0, releases: 0,
          metadataBytes: 0, payloadBytes: 0, storageBytes: 0, token: 0n};
        space.quota += grant.registryEntries;
        const {token, index: entry} = mint(domain, KIND.domain, 0, domain, binding());
        domain.index = entry;
        domain.token = token;
        invocation.domains.push(domain);
        return token;
      },
      vec_new(carrier, capacity) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Building');
        if (domain.vec !== null || domain.issued !== 0) fault(domain, 'vec-order');
        if (capacity !== BigInt(grant.rows)) fault(domain, 'capacity');
        reserve(domain, 1);
        if (invocation.faults.vec) return 0n;
        // Ordinary row Vec backing at its own allocation site; no Bytes event.
        const {views: storage, bytes} = carve(domain, rowLayout(grant.rows));
        const vec = {storage, len: 0, capacity: grant.rows};
        domain.vec = vec;
        domain.storageBytes = bytes;
        vec.handle = mint(domain, KIND.vec, 0, vec, binding()).token;
        return vec.handle;
      },
      string_literal(carrier, offset, length) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Building');
        const staged = domain.staged;
        if (staged.text !== null || (grant.textFirst ? staged.owner !== null : staged.owner === null))
          fault(domain, 'operand-order');
        const buffer = memory.buffer;
        if (!Number.isInteger(offset) || !Number.isInteger(length) || offset < LITERAL_BASE || length < 0
            || offset > buffer.byteLength - length) fault(domain, 'literal-range');
        const literal = new Uint8Array(buffer, offset, length);
        if (!validUtf8(literal)) fault(domain, 'utf8');
        reserve(domain, 1);
        // Ordinary String: a complete owned copy charged to String counters.
        const bytes = new Uint8Array(literal);
        invocation.ledger.strings++;
        invocation.ledger.stringBytes += length;
        domain.strings++;
        staged.text = mint(domain, KIND.string, 0, {bytes, contained: false}, binding()).token;
        return staged.text;
      },
      issue(carrier, ordinal) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Building');
        const staged = domain.staged;
        if (domain.vec === null) fault(domain, 'vec-order');
        if (ordinal !== BigInt(domain.issued) || domain.issued >= grant.rows) fault(domain, 'issue-range');
        if (domain.issued > 0 && !domain.tables.published[domain.issued - 1]) fault(domain, 'unpublished');
        if (staged.owner !== null || (grant.textFirst ? staged.text === null : staged.text !== null))
          fault(domain, 'operand-order');
        reserve(domain, 1);
        if (domain.issued === 0) {
          // First original Bytes operand: check the account carve and the
          // ordinary event/extent charge together, before any physical
          // allocation; then carve, charge and allocate. Nothing is refunded.
          const ledger = invocation.ledger, extent = grant.rows * grant.width;
          const account = invocation.account;
          if (account.offset + grant.metadataBytes > account.buffer.byteLength) fault(domain, 'metadata');
          if (ledger.events + 1 > MAX_EVENTS || ledger.cumulative + extent > MAX_CUMULATIVE)
            fault(domain, 'ordinary-byte-capacity');
          const {views: tables, bytes} = carve(domain, tableLayout(grant.rows));
          if (bytes !== grant.metadataBytes) throw new Error('selected metadata layout invariant');
          ledger.events += 1;
          ledger.fusedEvents += 1;
          ledger.cumulative += extent;
          domain.tables = tables;
          domain.metadataBytes = bytes;
          // Physical payload failure is invariant fail-stop, never a status.
          if (invocation.faults.payload) throw new Error('selected-fused:physical-payload-fault');
          domain.payload = new Uint8Array(extent);
          domain.payloadBytes = domain.payload.byteLength;
          domain.allocations++;
        }
        const slot = domain.issued, tables = domain.tables;
        const {token} = mint(domain, KIND.owner, slot, {contained: false}, binding());
        tables.nonce[slot] = token;
        tables.generation[slot] = 1n;
        tables.live[slot] = 1;
        domain.issued++;
        domain.children++;
        staged.owner = token;
        return token;
      },
      row_push(carrier, vector, scalar, text, lease) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Building');
        const {handle: old, entry: vec} = held(domain, vector, KIND.vec);
        const string = held(domain, text, KIND.string);
        const child = owner(domain, lease);
        const staged = domain.staged;
        if (string.entry.contained || child.entry.contained || child.slot !== vec.len
            || domain.tables.published[child.slot] || staged.text !== string.handle
            || staged.owner !== child.handle) fault(domain, 'publication-order');
        // A refused push consumes nothing; the plan's failure exit owns the row.
        if (vec.len >= vec.capacity || invocation.faults.push === child.slot) return 0n;
        reserve(domain, 1);
        vec.storage.scalar[vec.len] = scalar;
        vec.storage.text[vec.len] = string.handle;
        vec.storage.owner[vec.len] = child.handle;
        string.entry.contained = true;
        child.entry.contained = true;
        domain.tables.published[child.slot] = 1;
        vec.len++;
        domain.staged = {text: null, owner: null};
        vec.handle = renew(domain, KIND.vec, 0, vec, old);
        return vec.handle;
      },
      complete(carrier) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Building');
        if (domain.issued !== grant.rows || domain.vec === null || domain.vec.len !== grant.rows
            || domain.tables.published.includes(0)) fault(domain, 'unpublished');
        domain.phase = 'Complete';
        domain.pin = false;
        releaseIfLast(domain);
        return 0;
      },
      abort(carrier) {
        const domain = domainOf(carrier);
        phase(domain, 'Building');
        domain.phase = 'Aborted';
        return 0;
      },
      settle_abort(carrier) {
        const domain = domainOf(carrier);
        phase(domain, 'Aborted');
        if (domain.children !== 0 || domain.views !== 0 || domain.vec !== null || domain.strings !== 0
            || domain.clones !== 0) fault(domain, 'unsettled');
        domain.pin = false;
        releaseIfLast(domain);
        return 0;
      },
      vec_move(carrier, vector) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const {handle, entry: vec} = held(domain, vector, KIND.vec);
        if (domain.views !== 0) fault(domain, 'borrowed');
        vec.handle = renew(domain, KIND.vec, 0, vec, handle);
        return vec.handle;
      },
      vec_len(carrier, vector) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        return BigInt(held(domain, vector, KIND.vec).entry.len);
      },
      field_scalar(carrier, vector, position) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const vec = held(domain, vector, KIND.vec).entry;
        return vec.storage.scalar[index(domain, vec, position)];
      },
      field_text(carrier, vector, position) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const vec = held(domain, vector, KIND.vec).entry;
        const text = vec.storage.text[index(domain, vec, position)];
        held(domain, text, KIND.string);
        return text;
      },
      text_len(carrier, text) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        return BigInt(held(domain, text, KIND.string).entry.bytes.length);
      },
      text_byte(carrier, text, position) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const bytes = held(domain, text, KIND.string).entry.bytes, at = BigInt.asUintN(64, position);
        return at < BigInt(bytes.length) ? bytes[Number(at)] : -1;
      },
      payload_borrow(carrier, vector, position) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const vec = held(domain, vector, KIND.vec).entry;
        const child = owner(domain, vec.storage.owner[index(domain, vec, position)]);
        return borrowSlot(domain, child.slot);
      },
      view_len(carrier, borrowed) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        view(domain, borrowed);
        return BigInt(grant.width);
      },
      view_byte(carrier, borrowed, position) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const {slot} = view(domain, borrowed), at = BigInt.asUintN(64, position);
        return at < BigInt(grant.width) ? range(domain, slot)[Number(at)] : -1;
      },
      end_borrow(carrier, borrowed) {
        const domain = domainOf(carrier);
        const {handle, at, slot} = view(domain, borrowed);
        domain.tables.borrows[slot]--;
        domain.tables.viewLive[at] = 0;
        domain.views--;
        retire(domain, handle);
        releaseIfLast(domain);
        return 0;
      },
      sort(carrier, vector) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const {handle, entry: vec} = held(domain, vector, KIND.vec);
        if (domain.views !== 0) fault(domain, 'borrowed');
        reserve(domain, 1);
        const {scalar, text, owner: owners} = vec.storage;
        const textOf = at => space.objects[Number(text[at] & INDEX_MASK)].bytes;
        const payloadOf = at => range(domain, registry.target[Number(owners[at] & INDEX_MASK)]);
        const compare = (left, right) => {
          for (const role of grant.order) {
            const order = role === 'scalar'
              ? (scalar[left] < scalar[right] ? -1 : scalar[left] > scalar[right] ? 1 : 0)
              : role === 'text' ? compareBytes(textOf(left), textOf(right))
                : compareBytes(payloadOf(left), payloadOf(right));
            if (order !== 0) return order;
          }
          return 0;
        };
        // Stable in-place insertion sort: rows transport existing owners; no
        // allocation, copy, settlement or authority growth.
        for (let at = 1; at < vec.len; at++) {
          for (let probe = at; probe > 0 && compare(probe - 1, probe) > 0; probe--) {
            for (const array of [scalar, text, owners]) {
              const word = array[probe - 1];
              array[probe - 1] = array[probe];
              array[probe] = word;
            }
          }
        }
        vec.handle = renew(domain, KIND.vec, 0, vec, handle);
        return vec.handle;
      },
      vec_drop(carrier, vector) {
        const domain = domainOf(carrier);
        phase(domain, 'Complete', 'Aborted');
        const {handle, entry: vec} = held(domain, vector, KIND.vec);
        if (domain.views !== 0) fault(domain, 'borrowed');
        // Authenticate every row before releasing any, then release in
        // container order. A popped child is no longer contained here.
        for (let at = 0; at < vec.len; at++) {
          if (held(domain, vec.storage.text[at], KIND.string).entry.contained !== true
              || owner(domain, vec.storage.owner[at]).entry.contained !== true) fault(domain, 'contained');
        }
        for (let at = 0; at < vec.len; at++) {
          dropString(domain, vec.storage.text[at], true);
          dropOwner(domain, vec.storage.owner[at], true);
        }
        retire(domain, handle);
        domain.vec = null;
        releaseIfLast(domain);
        return 0;
      },
      string_drop(carrier, text) {
        const domain = domainOf(carrier);
        phase(domain, 'Complete', 'Aborted');
        dropString(domain, text, false);
        releaseIfLast(domain);
        return 0;
      },
      owner_drop(carrier, lease) {
        const domain = domainOf(carrier);
        phase(domain, 'Complete', 'Aborted');
        dropOwner(domain, lease, false);
        releaseIfLast(domain);
        return 0;
      },
      // The first selected failure is sticky; cleanup cannot replace it.
      select_failure(status) {
        if (invocation === null) refuse('no-invocation');
        if (invocation.selected === null) invocation.selected = status;
      },
      return_failure() {
        if (invocation === null || invocation.selected === null) refuse('no-selected-failure');
        throw new SelectedStatus(invocation.selected);
      },
    };

    // Host-side owner operations outside the lowered closed use domain. They
    // share every authentication, ledger and settlement path above.
    const extra = {
      vec_pop(carrier, vector) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const {handle, entry: vec} = held(domain, vector, KIND.vec);
        if (domain.views !== 0) fault(domain, 'borrowed');
        if (vec.len === 0) return [0n, handle];
        reserve(domain, 2);
        vec.len--;
        const text = vec.storage.text[vec.len], lease = vec.storage.owner[vec.len];
        held(domain, text, KIND.string).entry.contained = 'row';
        owner(domain, lease).entry.contained = 'row';
        const row = mint(domain, KIND.row, 0, {text, lease}, binding()).token;
        vec.handle = renew(domain, KIND.vec, 0, vec, handle);
        return [row, vec.handle];
      },
      row_drop(carrier, popped) {
        const domain = domainOf(carrier);
        phase(domain, 'Complete', 'Aborted');
        const {handle, entry} = held(domain, popped, KIND.row);
        const child = owner(domain, entry.lease);
        if (held(domain, entry.text, KIND.string).entry.contained !== 'row' || child.entry.contained !== 'row'
            || domain.tables.borrows[child.slot] !== 0) fault(domain, 'borrowed');
        dropString(domain, entry.text, 'row');
        dropOwner(domain, entry.lease, 'row');
        retire(domain, handle);
        releaseIfLast(domain);
        return 0;
      },
      row_borrow(carrier, popped) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const {entry} = held(domain, popped, KIND.row);
        return borrowSlot(domain, owner(domain, entry.lease).slot);
      },
      // Store renews exactly this child's nonce and generation before writing.
      store(carrier, popped, position, byte) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const {entry} = held(domain, popped, KIND.row);
        const child = owner(domain, entry.lease), tables = domain.tables;
        if (tables.borrows[child.slot] !== 0) fault(domain, 'borrowed');
        if (!Number.isInteger(position) || position < 0 || position >= grant.width
            || !Number.isInteger(byte) || byte < 0 || byte > 255) fault(domain, 'range');
        if (tables.generation[child.slot] === 0xffffffffffffffffn) fault(domain, 'generation-exhausted');
        const renewed = renew(domain, KIND.owner, child.slot, child.entry, child.handle);
        tables.nonce[child.slot] = renewed;
        tables.generation[child.slot]++;
        entry.lease = renewed;
        range(domain, child.slot)[position] = byte;
        return 0;
      },
      // Fresh ordinary Bytes with the ordinary event/payload debit before
      // allocation; only the authenticated range is copied. Never refunded.
      deep_clone(carrier, popped) {
        const domain = domainOf(carrier);
        active(domain); phase(domain, 'Complete');
        const {entry} = held(domain, popped, KIND.row);
        const child = owner(domain, entry.lease), ledger = invocation.ledger;
        if (ledger.events + 1 > MAX_EVENTS || ledger.cumulative + grant.width > MAX_CUMULATIVE)
          fault(domain, 'ordinary-byte-capacity');
        reserve(domain, 1);
        ledger.events += 1;
        ledger.cumulative += grant.width;
        const bytes = new Uint8Array(range(domain, child.slot));
        domain.clones++;
        return mint(domain, KIND.bytes, 0, {bytes}, binding()).token;
      },
      bytes_byte(carrier, clone, position) {
        const domain = domainOf(carrier);
        return held(domain, clone, KIND.bytes).entry.bytes[position] ?? -1;
      },
      bytes_drop(carrier, clone) {
        const domain = domainOf(carrier);
        const {handle} = held(domain, clone, KIND.bytes);
        retire(domain, handle);
        domain.clones--;
        releaseIfLast(domain);
        return 0;
      },
    };
    const controls = setup.controls === true ? Object.freeze({...env, ...extra}) : undefined;

    // Whole-invocation abandonment: release administrative backing and every
    // remaining entry without publishing a result or inventing finalizers.
    const abandon = domain => {
      for (let entry = 0; entry < space.entries; entry++) {
        if (registry.token[entry] !== 0n && registry.domain[entry] === domain.index && entry !== domain.index)
          retire(domain, registry.token[entry]);
      }
      if (domain.payload !== null) { domain.payload = null; domain.releases++; }
      domain.phase = 'Abandoned';
    };
    const snapshot = current => Object.freeze({
      ...current.ledger,
      selected: current.selected,
      domains: current.domains.map(domain => Object.freeze({phase: domain.phase, failure: domain.failure,
        allocations: domain.allocations, releases: domain.releases, payloadBytes: domain.payloadBytes,
        metadataBytes: domain.metadataBytes, issued: domain.issued, peakViews: domain.peakViews,
        storageBytes: domain.storageBytes})),
      firstSequence: current.firstSequence, nextSequence: space.sequence,
      liveEntries: space.entries - space.freeTop, outstandingQuota: space.quota,
    });
    function session(run, body) {
      if (invocation !== null) refuse('reentrant-invocation');
      const account = accounts.get(run.account);
      if (account === undefined) refuse('account');
      if (account.held !== null) refuse('account-held');
      const ordinaryEvents = run.ordinaryEvents ?? 0, ordinaryBytes = run.ordinaryBytes ?? 0;
      if (!Number.isInteger(ordinaryEvents) || ordinaryEvents < 0 || ordinaryEvents > MAX_EVENTS
          || !Number.isInteger(ordinaryBytes) || ordinaryBytes < 0 || ordinaryBytes > MAX_CUMULATIVE)
        throw new TypeError('selected ledger preload');
      const current = {serial: ++space.invocations, domains: [], account, selected: null,
        faults: Object.freeze({...(run.faults ?? {})}), firstSequence: space.sequence,
        ledger: {events: ordinaryEvents, cumulative: ordinaryBytes, fusedEvents: 0, accountBytes:
          account.buffer.byteLength, accountCarved: 0, strings: 0, stringBytes: 0, stringDrops: 0}};
      account.held = current;
      account.offset = 0;
      invocation = current;
      let value, status = 0, error = null;
      try { value = body(controls); }
      catch (failure) { if (failure instanceof SelectedStatus) status = failure.status; else error = failure; }
      // Every domain must settle inside its invocation; a remaining child is a
      // public or cross-invocation escape and publishes no result.
      let escaped = false;
      for (const domain of current.domains) {
        if (domain.phase !== 'Settled') { escaped = true; abandon(domain); }
        retire(domain, domain.token);
        releaseQuota(domain);
      }
      invocation = null;
      account.held = null;
      const observation = snapshot(current);
      if (error === null && escaped) error = new SelectedRefusal('unsettled-escape');
      if (error !== null) { error.observation = observation; throw error; }
      return Object.freeze({status, value: status === 0 ? value : undefined, observation});
    }
    const wasm = await WebAssembly.instantiate(module, {[NAMESPACE]: Object.freeze(env)});
    memory = wasm.exports.memory;
    return Object.freeze({
      invoke: run => session(run, () => wasm.exports.semaprax_main()),
      session: (run, body) => session(run, body),
    });
  }
  return Object.freeze({instantiate});
}
