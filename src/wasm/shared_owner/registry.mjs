// Private authenticated shared-owner registry for ordinary Core-Wasm Bytes.
//
// OPT #746 consumer closure, docs/OWNED-BYTE-PARTITION-V1.md. One backing
// record owns the original whole-owner allocation; each child lease owns one
// immutable disjoint range of it. The guest still sees the frozen two-word
// owned carrier `((0x80000000 | token) << 32) | extent`: tokens are opaque,
// minted by the enclosing host's single monotonic counter and resolved only
// through this registry's private table. No pointer, offset or header ever
// crosses into the guest, and nothing here probes memory to classify a value.
//
// The registry is not a producer. Partition preparation is reachable only
// through the enclosing host's witness facade; no Wasm import calls it until
// the cross-backend source/HIR join admits `bytes_partition`.

export const SHARED_OWNER_DOMAIN = "semaprax.private.wasm-shared-byte-owner.v1";
// Child metadata is bounded by the existing Vec<Bytes> carrier capacity
// (`vec_ops::MAX_CAPACITY`); the registry never raises it.
export const MAX_LEASE_SLOTS = 8192;
// The existing individual Bytes extent (`vec_ops::MAX_OWNED_PAYLOAD_BYTES`).
export const MAX_BYTES_EXTENT = 131072;
// A store renews its lease generation. Exhaustion refuses the store before
// any byte is written; tokens are never reissued, so there is no ABA reuse.
export const MAX_LEASE_GENERATION = 0xffff;
export const MAX_TOKEN = 0x7fffffff;
const U64_MAX = (1n << 64n) - 1n;

// Partition refusals in the proposed `semaprax.byte-partition.v1` domain.
// Numeric codes are unassigned by the owning contract; the registry reports
// stable symbols only and never invents a local code.
export const PARTITION_REFUSALS = Object.freeze([
  "width_zero",
  "extent_overflow",
  "extent_mismatch",
  "metadata_capacity",
]);

export function carrierOf(token, extent) {
  if (!Number.isInteger(token) || token < 1 || token > MAX_TOKEN
      || !Number.isInteger(extent) || extent < 0 || extent > MAX_BYTES_EXTENT) {
    throw new Error("SEMAPRAX shared owner carrier invariant");
  }
  return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(extent));
}

export function decodeOwnedCarrier(carrier) {
  if (typeof carrier !== "bigint") throw new TypeError("SEMAPRAX byte carrier is not i64");
  const word = BigInt.asUintN(64, carrier);
  const extent = Number(word & 0xffffffffn);
  const root = Number((word >> 32n) & 0xffffffffn);
  return { word, extent, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff };
}

// `tokens` is the enclosing host's single never-reset token authority:
// `remaining()` reports how many fresh tokens `mint()` can still return.
export function createSharedByteOwnerRegistry({ tokens, maxLeaseSlots = MAX_LEASE_SLOTS, maxBackings = 1 } = {}) {
  if (tokens === null || typeof tokens !== "object"
      || typeof tokens.mint !== "function" || typeof tokens.remaining !== "function") {
    throw new TypeError("SEMAPRAX shared owner token authority is required");
  }
  if (!Number.isSafeInteger(maxLeaseSlots) || maxLeaseSlots < 1 || maxLeaseSlots > MAX_LEASE_SLOTS) {
    throw new RangeError("invalid SEMAPRAX shared owner lease-slot bound");
  }
  // Child repartitioning is closed, and one whole-backing partition is the
  // only proven metadata shape. A larger backing count needs its own proof.
  if (!Number.isSafeInteger(maxBackings) || maxBackings < 1 || maxBackings > 1) {
    throw new RangeError("invalid SEMAPRAX shared owner backing bound");
  }
  // Unforgeable issuer identity: only records minted here carry it.
  const issuer = Object.freeze(Object.create(null));
  const leases = new Map();
  const tickets = new WeakMap();
  let reservedSlots = 0;
  let liveLeases = 0;
  let peakLiveLeases = 0;
  let liveBackings = 0;
  let reservedBackings = 0;
  let releasedBackings = 0;
  let nextBacking = 1;

  const mintFresh = () => {
    const token = tokens.mint();
    if (!Number.isInteger(token) || token < 1 || token > MAX_TOKEN || leases.has(token)) {
      throw new Error("SEMAPRAX shared owner token authority invariant");
    }
    return token;
  };

  // Authenticates one carrier against the current lease generation. Every
  // consumer resolves through here before obtaining any backing bytes.
  const authenticate = decoded => {
    if (!decoded.tagged || decoded.token === 0) return null;
    const lease = leases.get(decoded.token);
    if (lease === undefined) return null;
    const backing = lease.backing;
    if (lease.issuer !== issuer || backing.issuer !== issuer || lease.token !== decoded.token
        || !lease.live || backing.released || backing.bytes === null) {
      throw new Error("SEMAPRAX shared owner lease authority invariant");
    }
    if (decoded.extent !== lease.extent) {
      throw new Error("SEMAPRAX shared owner lease extent invariant");
    }
    return lease;
  };

  const requireLease = carrier => {
    const lease = authenticate(decodeOwnedCarrier(carrier));
    if (lease === null) throw new Error("SEMAPRAX stale, foreign or unleased Bytes carrier");
    return lease;
  };

  // Validation order: width, checked extent, exact extent, then complete
  // metadata. No state changes; the caller still owns the whole input.
  const prepare = (wholeToken, whole, count, width) => {
    if (!Number.isInteger(wholeToken) || wholeToken < 1 || wholeToken > MAX_TOKEN) {
      throw new Error("SEMAPRAX shared owner partition input invariant");
    }
    // A child, even one at offset zero covering the whole backing extent, is
    // never an eligible partition input.
    if (leases.has(wholeToken)) {
      throw new Error("SEMAPRAX shared owner child partition refused");
    }
    if (!(whole instanceof Uint8Array) || whole.byteLength > MAX_BYTES_EXTENT
        || whole.byteOffset !== 0 || whole.byteLength !== whole.buffer.byteLength) {
      throw new Error("SEMAPRAX shared owner whole backing invariant");
    }
    if (typeof count !== "bigint" || typeof width !== "bigint"
        || count < 0n || count > U64_MAX || width < 0n || width > U64_MAX) {
      throw new TypeError("SEMAPRAX shared owner partition operand invariant");
    }
    if (width === 0n) return { refused: "width_zero" };
    const extent = count * width;
    if (extent > U64_MAX) return { refused: "extent_overflow" };
    if (extent !== BigInt(whole.byteLength)) return { refused: "extent_mismatch" };
    const children = Number(count);
    if (count > BigInt(MAX_LEASE_SLOTS)
        || children > maxLeaseSlots - reservedSlots
        || (children > 0 && reservedBackings >= maxBackings)
        || tokens.remaining() < children) {
      return { refused: "metadata_capacity" };
    }
    reservedSlots += children;
    if (children > 0) reservedBackings += 1;
    const ticket = Object.freeze(Object.create(null));
    tickets.set(ticket, { state: "prepared", wholeToken, whole, children, width: Number(width) });
    return { ticket };
  };

  const pending = ticket => {
    const prepared = tickets.get(ticket);
    if (prepared === undefined || prepared.state !== "prepared") {
      throw new Error("SEMAPRAX shared owner partition ticket invariant");
    }
    return prepared;
  };

  // Releases only the unpublished reservation; the whole owner is untouched.
  const abort = ticket => {
    const prepared = pending(ticket);
    prepared.state = "aborted";
    reservedSlots -= prepared.children;
    if (prepared.children > 0) reservedBackings -= 1;
    prepared.whole = null;
  };

  // The caller removes the whole owner from its table first; every check and
  // reservation already succeeded, so commit cannot fail after that point.
  const commit = ticket => {
    const prepared = pending(ticket);
    prepared.state = "committed";
    const { whole, children, width } = prepared;
    prepared.whole = null;
    if (children === 0) return [];
    const backing = {
      issuer,
      id: nextBacking++,
      bytes: whole,
      extent: whole.byteLength,
      live: children,
      released: false,
    };
    liveBackings += 1;
    const carriers = new Array(children);
    for (let index = 0; index < children; index += 1) {
      const token = mintFresh();
      const offset = index * width;
      const lease = {
        issuer,
        backing,
        token,
        index,
        offset,
        extent: width,
        generation: 1,
        live: true,
        view: whole.subarray(offset, offset + width),
      };
      leases.set(token, lease);
      carriers[index] = carrierOf(token, width);
    }
    liveLeases += children;
    if (liveLeases > peakLiveLeases) peakLiveLeases = liveLeases;
    return carriers;
  };

  const settle = lease => {
    lease.live = false;
    lease.view = null;
    leases.delete(lease.token);
    liveLeases -= 1;
    reservedSlots -= 1;
    const backing = lease.backing;
    backing.live -= 1;
    if (backing.live > 0) return false;
    // Last live lease: the backing is released exactly once.
    if (backing.released) throw new Error("SEMAPRAX shared owner backing release invariant");
    backing.released = true;
    backing.bytes = null;
    liveBackings -= 1;
    reservedBackings -= 1;
    releasedBackings += 1;
    return true;
  };

  return Object.freeze({
    domain: SHARED_OWNER_DOMAIN,
    // True only for a live lease of this registry; whole owners and foreign
    // tokens are left to the caller's own table, which rejects them.
    owns(carrier) {
      return authenticate(decodeOwnedCarrier(carrier)) !== null;
    },
    // Bounded view of exactly the authenticated range. Typed-array bounds
    // are the read/write fence: an index outside the range cannot reach a
    // sibling's bytes.
    view(carrier) {
      return requireLease(carrier).view;
    },
    // A renewed store keeps the exact range and advances the generation.
    // `mutate` validates its operands before writing; it runs only after
    // renewal capacity is proven, and its failure leaves the old generation.
    store(carrier, mutate) {
      const lease = requireLease(carrier);
      if (lease.generation >= MAX_LEASE_GENERATION) {
        throw new Error("SEMAPRAX shared owner lease generation exhausted");
      }
      if (tokens.remaining() < 1) throw new Error("SEMAPRAX owned Bytes token space exhausted");
      mutate(lease.view);
      const token = mintFresh();
      leases.delete(lease.token);
      lease.token = token;
      lease.generation += 1;
      leases.set(token, lease);
      return carrierOf(token, lease.extent);
    },
    // Ordinary drop of one child. Returns whether this was the final lease
    // and therefore the single backing release.
    release(carrier) {
      return settle(requireLease(carrier));
    },
    // Public copy-out plus provider drop: a fresh unshared snapshot, then
    // the private lease settles. No interior view or offset is returned.
    copyOutAndRelease(carrier) {
      const lease = requireLease(carrier);
      const snapshot = new Uint8Array(lease.extent);
      snapshot.set(lease.view);
      settle(lease);
      return snapshot;
    },
    prepare,
    commit,
    abort,
    // Context abandonment after a trap: releases remaining administration
    // without synthesizing source finalizer order or publishing a result.
    abandon() {
      let released = 0;
      for (const lease of [...leases.values()]) if (settle(lease)) released += 1;
      return released;
    },
    // Independent inventory audit: every live range lies inside its live
    // backing, live counts agree, and no two live ranges overlap.
    audit() {
      const byBacking = new Map();
      for (const [token, lease] of leases) {
        const backing = lease.backing;
        if (lease.token !== token || !lease.live || backing.released || lease.issuer !== issuer
            || lease.offset < 0 || lease.extent < 0 || lease.offset + lease.extent > backing.extent
            || lease.view === null || lease.view.byteLength !== lease.extent) {
          throw new Error("SEMAPRAX shared owner audit: malformed lease");
        }
        if (!byBacking.has(backing)) byBacking.set(backing, []);
        byBacking.get(backing).push(lease);
      }
      for (const [backing, list] of byBacking) {
        if (list.length !== backing.live) throw new Error("SEMAPRAX shared owner audit: live count");
        const ordered = [...list];
        ordered.sort((a, b) => a.offset - b.offset);
        for (let i = 1; i < ordered.length; i += 1) {
          const previous = ordered[i - 1];
          if (previous.offset + previous.extent > ordered[i].offset
              && previous.extent > 0 && ordered[i].extent > 0) {
            throw new Error("SEMAPRAX shared owner audit: overlapping live ranges");
          }
        }
      }
      if (byBacking.size !== liveBackings || leases.size !== liveLeases) {
        throw new Error("SEMAPRAX shared owner audit: inventory");
      }
      return true;
    },
    stats() {
      return Object.freeze({
        liveLeases,
        peakLiveLeases,
        liveBackings,
        releasedBackings,
        reservedSlots,
      });
    },
  });
}
