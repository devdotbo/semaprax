// Private strict Core-Wasm host for ordinary Bytes, Vec<Bytes> and Bytes
// iterators with lease-aware consumers (OPT #746 consumer closure).
//
// Every import keeps the frozen ordinary `env` name, signature and two-word
// carrier. A carrier resolves either to an ordinary whole owner in `entries`
// or to a live lease in the shared-owner registry; nothing else is readable,
// writable or droppable. This is not the public browser adapter: public
// routes keep their own token tables and refuse lease carriers as stale.
// Admission is closed: no `env` import issues a lease. The witness facade
// `partitionForWitness` exists only for the owning hostile regressions until
// the cross-backend join admits `bytes_partition`.
import {
  MAX_BYTES_EXTENT,
  MAX_TOKEN,
  carrierOf,
  createSharedByteOwnerRegistry,
  decodeOwnedCarrier,
} from "./registry.mjs";

// Existing source-derived limits, mirrored as defense in depth. They are not
// raised, and a partition charges neither: it issues leases over a backing
// that its constructor already paid for.
export const MAX_BACKING_EVENTS = 32;
export const MAX_PAYLOAD_BYTES = 2097152;
export const MAX_VEC_CAPACITY = 8192n;
const OWNED_BYTES_TAG = 9;
const FIXED_MEMORY_BYTES = 131072;

const TOKEN_AUTHORITY_KEY = Symbol.for("semaprax.private.wasm-shared-byte-owner.tokens.v1");

// One realm-wide never-reset token space for every private host, so a carrier
// minted by another host or instance can never name a live entry here.
function realmTokenAuthority() {
  const installed = globalThis[TOKEN_AUTHORITY_KEY];
  if (installed !== undefined) {
    if (installed === null || typeof installed !== "object" || !Object.isFrozen(installed)
        || typeof installed.mint !== "function" || typeof installed.remaining !== "function") {
      throw new Error("SEMAPRAX private Bytes token authority global is invalid");
    }
    return installed;
  }
  let next = 1;
  const authority = Object.freeze({
    remaining: () => MAX_TOKEN - next + 1,
    mint: () => {
      if (next > MAX_TOKEN) throw new Error("SEMAPRAX owned Bytes token space exhausted");
      return next++;
    },
  });
  Object.defineProperty(globalThis, TOKEN_AUTHORITY_KEY, {
    value: authority,
    configurable: false,
    enumerable: false,
    writable: false,
  });
  return authority;
}

export function createPrivateOrdinaryByteHost(options = {}) {
  let instance = null;
  const tokens = realmTokenAuthority();
  const shared = createSharedByteOwnerRegistry({ tokens, maxLeaseSlots: options.maxLeaseSlots });
  const entries = new Map();
  const vectors = new Map();
  const iterators = new Map();
  let nextVec = 1n;
  let nextIter = 1n << 62n;
  let events = 0;
  let payload = 0;
  let copies = 0;
  let drops = 0;

  const memory = () => {
    const candidate = instance?.exports.__spx_byte_memory ?? instance?.exports.memory;
    if (!(candidate instanceof WebAssembly.Memory)) throw new Error("SEMAPRAX byte memory invariant");
    return candidate;
  };
  // Ordinary allocation-producing event: charged before the backing exists.
  const charge = length => {
    if (events >= MAX_BACKING_EVENTS || payload > MAX_PAYLOAD_BYTES - length) {
      throw new Error("SEMAPRAX ordinary Bytes allocation budget exceeded");
    }
    events += 1;
    payload += length;
  };
  const own = bytes => {
    if (!(bytes instanceof Uint8Array) || bytes.byteLength > MAX_BYTES_EXTENT) {
      throw new Error("SEMAPRAX owned Bytes length invariant");
    }
    if (tokens.remaining() < 1) throw new Error("SEMAPRAX owned Bytes token space exhausted");
    charge(bytes.byteLength);
    // Fresh exact backing: clone and copy never alias a lease or its backing.
    const owned = new Uint8Array(bytes.byteLength);
    owned.set(bytes);
    const token = tokens.mint();
    entries.set(token, owned);
    copies += 1;
    return carrierOf(token, owned.byteLength);
  };
  const whole = decoded => {
    const entry = entries.get(decoded.token);
    if (!(entry instanceof Uint8Array) || entry.byteLength !== decoded.extent) return null;
    return entry;
  };
  // Owned carrier: whole owner, else authenticated lease, else rejection.
  const owned = carrier => {
    const decoded = decodeOwnedCarrier(carrier);
    if (!decoded.tagged || decoded.token === 0) throw new Error("SEMAPRAX owned Bytes token invariant");
    const entry = whole(decoded);
    if (entry !== null) return entry;
    if (shared.owns(carrier)) return shared.view(carrier);
    throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
  };
  // Borrowed read: owned carrier, checked range descriptor, or fixed root.
  const read = carrier => {
    const decoded = decodeOwnedCarrier(carrier);
    if (decoded.tagged) return owned(carrier);
    const bytes = new Uint8Array(memory().buffer);
    if ((decoded.root & 0xc0000000) === 0x40000000) {
      const pointer = (decoded.root & 0xffff) * 8;
      if (pointer > FIXED_MEMORY_BYTES - 32) throw new Error("SEMAPRAX byte range descriptor bounds invariant");
      const descriptor = new DataView(bytes.buffer, pointer, 32);
      const identity = descriptor.getUint32(0, true);
      if (identity === 0 || identity !== ((decoded.root >>> 16) & 0x1fff)
          || descriptor.getUint32(4, true) !== pointer) {
        throw new Error("SEMAPRAX byte range descriptor identity invariant");
      }
      const ultimate = descriptor.getBigInt64(8, true);
      const root = decodeOwnedCarrier(ultimate);
      const start = descriptor.getBigUint64(16, true);
      const extent = descriptor.getBigUint64(24, true);
      if ((root.root & 0xc0000000) === 0x40000000 || extent !== BigInt(decoded.extent)
          || start > BigInt(root.extent) || extent > BigInt(root.extent) - start) {
        throw new Error("SEMAPRAX byte range descriptor extent invariant");
      }
      // A view over a lease resolves through the lease's current generation
      // and can only narrow its range.
      return read(ultimate).subarray(Number(start), Number(start + extent));
    }
    if (decoded.root > bytes.byteLength - decoded.extent) throw new Error("SEMAPRAX fixed byte range invariant");
    return bytes.subarray(decoded.root, decoded.root + decoded.extent);
  };
  // Ordinary drop: a whole owner frees its entry; a lease settles once and
  // the last live lease of a backing releases it exactly once.
  const drop = carrier => {
    const decoded = decodeOwnedCarrier(carrier);
    if (!decoded.tagged) throw new Error("SEMAPRAX borrowed Bytes drop invariant");
    if (whole(decoded) !== null) {
      entries.delete(decoded.token);
    } else if (shared.owns(carrier)) {
      shared.release(carrier);
    } else {
      throw new Error("SEMAPRAX double or stale Bytes drop");
    }
    drops += 1;
  };
  // Stores validate every operand before writing. A whole owner keeps its
  // carrier; a lease renews its generation over the same exact range.
  const store = (carrier, validate, write) => {
    const decoded = decodeOwnedCarrier(carrier);
    if (!decoded.tagged) throw new Error("SEMAPRAX borrowed Bytes store invariant");
    const entry = whole(decoded);
    if (entry !== null) {
      validate(entry);
      write(entry);
      return BigInt.asIntN(64, decoded.word);
    }
    if (!shared.owns(carrier)) throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
    return shared.store(carrier, view => { validate(view); write(view); });
  };
  const byte = value => Number.isInteger(value) && value >= 0 && value <= 255;
  const interval = (index, width) => target => {
    if (typeof index !== "bigint" || index < 0n || index > BigInt(target.byteLength)
        || BigInt(target.byteLength) - index < BigInt(width)) {
      throw new Error("SEMAPRAX owned byte buffer interval invariant");
    }
  };
  const choice = (carrier, index, one, sourceCarrier, selector, width, extended) => {
    if (typeof selector !== "bigint" || !byte(one)) throw new Error("SEMAPRAX owned byte buffer choice invariant");
    const bits = BigInt.asUintN(64, selector);
    const copy = (bits & (1n << 63n)) !== 0n;
    const wide = extended && (bits & (1n << 62n)) !== 0n;
    const start = bits & ((1n << (extended ? 62n : 63n)) - 1n);
    const count = copy ? (wide ? 48 : width) : 1;
    // The source is read before the destination is written, so a source that
    // is the destination's own range observes its pre-store bytes.
    const input = read(sourceCarrier);
    const values = new Uint8Array(count);
    if (copy) {
      for (let offset = 0; offset < count; offset += 1) {
        const at = start + BigInt(offset);
        values[offset] = at < BigInt(input.byteLength) ? input[Number(at)] : 0;
      }
    } else {
      values[0] = one;
    }
    return store(carrier, interval(index, count), target => target.set(values, Number(index)));
  };

  const vector = (handle, tag) => {
    const value = vectors.get(handle);
    if (value === undefined || tag !== value.tag || tag < 1 || tag > 9) throw new Error("SEMAPRAX stale or mistyped Vec");
    return value;
  };
  // Every successful Vec mutation renews the Vec authority.
  const renewVec = (handle, value) => {
    vectors.delete(handle);
    const renewed = nextVec++;
    vectors.set(renewed, value);
    return renewed;
  };
  const outputView = (out, length) => {
    if (!Number.isInteger(out) || out < 0 || out > memory().buffer.byteLength - length) return null;
    return new DataView(memory().buffer, out, length);
  };
  const iterator = (handle, cursor) => {
    const state = iterators.get(handle);
    if (state === undefined || typeof cursor !== "bigint" || cursor !== state.cursor
        || cursor > BigInt(state.values.length)) return null;
    return state;
  };
  const utf8 = bytes => {
    for (let index = 0; index < bytes.length;) {
      const first = bytes[index++];
      let extra, minimum, scalar;
      if (first < 128) continue;
      if (first >= 194 && first <= 223) { extra = 1; minimum = 128; scalar = first & 31; }
      else if (first >= 224 && first <= 239) { extra = 2; minimum = 2048; scalar = first & 15; }
      else if (first >= 240 && first <= 244) { extra = 3; minimum = 65536; scalar = first & 7; }
      else return false;
      if (index + extra > bytes.length) return false;
      for (let count = 0; count < extra; count += 1) {
        const next = bytes[index++];
        if ((next & 192) !== 128) return false;
        scalar = (scalar << 6) | (next & 63);
      }
      if (scalar < minimum || scalar > 1114111 || (scalar >= 55296 && scalar <= 57343)) return false;
    }
    return true;
  };
  const text = carrier => {
    const bytes = read(carrier);
    if (!utf8(bytes)) throw new Error("SEMAPRAX String UTF-8 invariant");
    return bytes;
  };
  const textOutput = offset => {
    if (!Number.isInteger(offset) || offset < 65536 || offset % 8 !== 0 || offset > FIXED_MEMORY_BYTES - 8) {
      throw new Error("SEMAPRAX checked text output slot invariant");
    }
    return new DataView(memory().buffer, offset, 8);
  };

  const imports = Object.freeze({
    spx_bytes_zeroed: count => {
      if (typeof count !== "bigint" || count < 0n || count > BigInt(MAX_BYTES_EXTENT)) {
        throw new Error("SEMAPRAX owned byte buffer capacity invariant");
      }
      return own(new Uint8Array(Number(count)));
    },
    // Clone and nonconsuming copy deep-copy only the authenticated range into
    // fresh ordinary backing with the ordinary event/payload charge.
    spx_bytes_copy: carrier => own(read(carrier)),
    spx_bytes_get: (carrier, index) => {
      const bytes = read(carrier);
      const unsigned = BigInt.asUintN(64, index);
      return unsigned >= BigInt(bytes.byteLength) ? -1 : bytes[Number(unsigned)];
    },
    spx_bytes_as_slice: carrier => {
      read(carrier);
      return carrier;
    },
    spx_bytes_drop: drop,
    spx_bytes_set: (carrier, index, value) => store(carrier, target => {
      if (typeof index !== "bigint" || index < 0n || index >= BigInt(target.byteLength) || !byte(value)) {
        throw new Error("SEMAPRAX owned byte buffer element invariant");
      }
    }, target => { target[Number(index)] = value; }),
    spx_bytes_set5: (carrier, index, a, b, c, d, e) => {
      const values = [a, b, c, d, e];
      if (!values.every(byte)) throw new Error("SEMAPRAX owned byte buffer five-byte interval invariant");
      return store(carrier, interval(index, 5), target => target.set(values, Number(index)));
    },
    spx_bytes_set1_or5: (carrier, index, one, source, selector) =>
      choice(carrier, index, one, source, selector, 5, false),
    spx_bytes_set1_or6_or48: (carrier, index, one, source, selector) =>
      choice(carrier, index, one, source, selector, 6, true),

    spx_string_from_utf8_v1: (carrier, offset) => {
      const bytes = read(carrier), output = textOutput(offset);
      if (!utf8(bytes)) return 21;
      output.setBigInt64(0, own(bytes), true);
      return 0;
    },
    spx_string_len_chars_v1: carrier => {
      let count = 0n;
      for (const value of text(carrier)) if ((value & 192) !== 128) count += 1n;
      return count;
    },
    spx_string_from_str_v2: (carrier, offset) => {
      const bytes = read(carrier), output = textOutput(offset);
      if (!utf8(bytes)) return 25;
      output.setBigInt64(0, own(bytes), true);
      return 0;
    },
    spx_string_starts_with_v1: (value, prefix) => {
      const a = text(value), b = text(prefix);
      if (b.byteLength > a.byteLength) return 0;
      for (let index = 0; index < b.byteLength; index += 1) if (a[index] !== b[index]) return 0;
      return 1;
    },
    spx_string_contains_v1: (value, needle) => {
      const a = text(value), b = text(needle);
      if (b.byteLength === 0) return 1;
      outer: for (let start = 0; start + b.byteLength <= a.byteLength; start += 1) {
        for (let index = 0; index < b.byteLength; index += 1) if (a[start + index] !== b[index]) continue outer;
        return 1;
      }
      return 0;
    },
    spx_string_compare_v2: (left, right) => {
      const a = text(left), b = text(right), bound = Math.min(a.byteLength, b.byteLength);
      for (let index = 0; index < bound; index += 1) {
        if (a[index] !== b[index]) return a[index] < b[index] ? -1n : 1n;
      }
      return a.byteLength === b.byteLength ? 0n : a.byteLength < b.byteLength ? -1n : 1n;
    },

    spx_vec_with_capacity_v2: (tag, capacity) => {
      if (!Number.isInteger(tag) || tag < 1 || tag > 9) throw new Error("SEMAPRAX Vec tag invariant");
      if (typeof capacity !== "bigint" || capacity < 0n || capacity > MAX_VEC_CAPACITY) return 0n;
      const handle = nextVec++;
      vectors.set(handle, { tag, capacity, values: [] });
      return handle;
    },
    spx_vec_push_v2: (handle, tag, item) => {
      const value = vector(handle, tag);
      if (tag === OWNED_BYTES_TAG) owned(item);
      if (BigInt(value.values.length) >= value.capacity) return 0n;
      value.values.push(item);
      return renewVec(handle, value);
    },
    spx_vec_len_v2: (handle, tag) => BigInt(vector(handle, tag).values.length),
    spx_vec_capacity_v2: (handle, tag) => vector(handle, tag).capacity,
    spx_vec_get_v2: (handle, tag, index) => {
      const value = vector(handle, tag);
      if (tag === OWNED_BYTES_TAG) throw new Error("SEMAPRAX owned Vec payload cannot be copied");
      return value.values[Number(index)];
    },
    // Canonical element order; each lease settles independently, so a child
    // already transferred out by iteration is unaffected.
    spx_vec_drop_v2: handle => {
      const raw = vectors.get(handle);
      if (raw === undefined) throw new Error("SEMAPRAX stale Vec drop");
      const value = vector(handle, raw.tag);
      vectors.delete(handle);
      if (value.tag === OWNED_BYTES_TAG) for (const item of value.values) drop(item);
    },
    spx_vec_reserve_exact_v2: (handle, tag, additional) => {
      const value = vector(handle, tag);
      if (typeof additional !== "bigint" || additional < 0n) return 0n;
      const target = BigInt(value.values.length) + additional;
      if (target > MAX_VEC_CAPACITY) return 0n;
      if (target > value.capacity) value.capacity = target;
      return renewVec(handle, value);
    },
    spx_vec_set_v2: (handle, tag, index, item) => {
      const value = vector(handle, tag);
      if (tag === OWNED_BYTES_TAG) owned(item);
      if (typeof index !== "bigint" || index < 0n || index >= BigInt(value.values.length)) return 0n;
      if (tag === OWNED_BYTES_TAG) drop(value.values[Number(index)]);
      value.values[Number(index)] = item;
      return renewVec(handle, value);
    },
    spx_vec_clear_v2: (handle, tag) => {
      const value = vector(handle, tag);
      const old = value.values;
      value.values = [];
      if (tag === OWNED_BYTES_TAG) for (const item of old) drop(item);
      return renewVec(handle, value);
    },

    // Consuming traversal: the Vec authority ends and the iterator owns the
    // untransferred suffix.
    spx_iter_bytes_into_v2: (handle, out) => {
      let value;
      try { value = vector(handle, OWNED_BYTES_TAG); } catch (_) { return 2; }
      const view = outputView(out, 16);
      if (view === null) return 2;
      const successor = nextIter++;
      vectors.delete(handle);
      iterators.set(successor, { values: value.values, cursor: 0n });
      view.setBigUint64(0, successor, true);
      view.setBigUint64(8, 0n, true);
      return 0;
    },
    // Next transfers the original child carrier, never a copy; the popped
    // child survives the iterator, the Vec and every sibling.
    spx_iter_bytes_next_v2: (handle, cursor, out) => {
      const state = iterator(handle, cursor);
      if (state === null) return 2;
      const view = outputView(out, 32);
      if (view === null) return 2;
      if (cursor === BigInt(state.values.length)) {
        iterators.delete(handle);
        for (let index = 0; index < 32; index += 1) view.setUint8(index, 0);
        return 0;
      }
      const item = state.values[Number(cursor)];
      try { owned(item); } catch (_) { return 2; }
      const successor = nextIter++;
      iterators.delete(handle);
      iterators.set(successor, { values: state.values, cursor: cursor + 1n });
      view.setUint32(0, 1, true);
      view.setUint32(4, 0, true);
      view.setBigUint64(8, BigInt.asUintN(64, item), true);
      view.setBigUint64(16, successor, true);
      view.setBigUint64(24, cursor + 1n, true);
      return 0;
    },
    // Abandonment drops only the untransferred suffix in canonical order.
    spx_iter_bytes_drop_v2: (handle, cursor) => {
      const state = iterator(handle, cursor);
      if (state === null) throw new Error("SEMAPRAX stale Iter drop");
      iterators.delete(handle);
      for (let index = Number(cursor); index < state.values.length; index += 1) drop(state.values[index]);
    },
  });

  const facade = Object.freeze({
    // Witness-only lease issuance for the owning regressions. Validation runs
    // before any transfer; refusal leaves the whole owner live and unchanged.
    partitionForWitness(carrier, count, width) {
      const decoded = decodeOwnedCarrier(carrier);
      if (!decoded.tagged) throw new Error("SEMAPRAX shared owner partition input invariant");
      const entry = whole(decoded);
      if (entry === null) {
        if (shared.owns(carrier)) throw new Error("SEMAPRAX shared owner child partition refused");
        throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
      }
      const prepared = shared.prepare(decoded.token, entry, count, width);
      if (prepared.refused !== undefined) return Object.freeze({ ok: false, refused: prepared.refused });
      entries.delete(decoded.token);
      return Object.freeze({ ok: true, children: Object.freeze(shared.commit(prepared.ticket)) });
    },
    // Builds a host-owned Vec<Bytes> over committed children, as the
    // producer's provisional result would after its own capacity check.
    vecForWitness(children) {
      if (!Array.isArray(children) || BigInt(children.length) > MAX_VEC_CAPACITY) {
        throw new Error("SEMAPRAX witness Vec invariant");
      }
      for (const item of children) owned(item);
      const handle = nextVec++;
      vectors.set(handle, { tag: OWNED_BYTES_TAG, capacity: BigInt(children.length), values: [...children] });
      return handle;
    },
    // Public copy-out plus provider drop for any owned carrier.
    copyOutAndDrop(carrier) {
      if (shared.owns(carrier)) {
        drops += 1;
        return shared.copyOutAndRelease(carrier);
      }
      const snapshot = new Uint8Array(owned(carrier));
      drop(carrier);
      return snapshot;
    },
    peek(carrier) {
      return new Uint8Array(read(carrier));
    },
    beginInvocation() {
      events = 0;
      payload = 0;
      copies = 0;
      drops = 0;
    },
    abandon() {
      vectors.clear();
      iterators.clear();
      entries.clear();
      return shared.abandon();
    },
    audit: () => shared.audit(),
    stats() {
      return Object.freeze({
        ...shared.stats(),
        entries: entries.size,
        vectors: vectors.size,
        iterators: iterators.size,
        events,
        payload,
        copies,
        drops,
      });
    },
  });

  return Object.freeze({
    imports,
    facade,
    bind(wasmInstance) {
      if (instance !== null) throw new Error("SEMAPRAX private ordinary host already bound");
      instance = wasmInstance;
    },
  });
}
