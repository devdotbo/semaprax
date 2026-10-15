### Record profile details

- Monomorphic acyclic records may own String/Bytes, Copy scalars and nested
  records. Pass with `own` or `borrow`; results own their leaves. Generic,
  resource/view/class and invariant-bearing String records are `SPX-T309`;
  see [String records](OWNED-STRING-RECORDS-V1.md).
- Whole-subtree binding (source implementation; qualification pending):
  `match own packet { Envelope { left, right, sequence } => sequence, }`
  moves each named record child without recursively unpacking it. Include every
  field exactly once; `_` cannot hide an owned subtree. The scrutinee must be a
  named unprojected owner with no live descendant loan. Complete hidden children
  still count toward limits; `match borrow` keeps recursive patterns.
  See [owned subtree patterns](NESTED-OWNED-RECORD-DESTRUCTURING-V2.md).
- Finite record loops (source implementation; qualification pending): an
  effect-free, parameter-free i64 function may use `let mut i = 0;` immediately
  before `while i < N`, with literal `N` in 1–32. Each body constructs one fresh
  immutable monomorphic record, then discards one owning record match and ends
  in `0`. Its single guard-free arm first increments that exact counter by one;
  branch tails may continue, break, return or complete normally. Extra arm
  statements, counter shadowing and nested loops refuse. Actual allocations
  still obey the existing 32-event/2 MiB limits. See
  [the exact admitted shape](FINITE-OWNED-RECORD-DISCARD-LOOPS-V1.md).
- Copy-record Vec admits identified flat records with 1–8 Copy fields. Project
  v30 adds `Vec<string>` and flat records with up to two direct owned
  String/Bytes fields. User records need `@id`; ask
  `help language author:copy-record-vec` or `help language author:owned-data`.
  See [Owned Leaf Collections v1](OWNED-LEAF-COLLECTIONS-V1.md).
