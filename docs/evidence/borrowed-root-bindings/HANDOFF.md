# Borrowed-root binding scopes: source-only handoff

Base: `eb02a89b6174bced05e02f6ede2ebcda55cd5893`.
Private branch: `codex/borrowed-root-bindings-eb02`.
Worktree: `/Volumes/Semaprax/worktrees/borrowed-root-bindings-eb02/AI-Lang`.

The root scope now owns one `Rc<Cow<BTreeMap>>` carrier and borrows the complete
caller map until a real declaration/pattern/traversal insertion requires the
complete map copy rather than borrowing. Already-owned unique scopes still
mutate in place; shared owned scopes still detach complete maps. Inserting a first
local can merely defer the original upfront copy, not eliminate it. Every
original map field and sorted-map lookup,
shadowing, mutation and sibling/parent isolation remains at its original owner.
Nested expression and input lifetimes are synchronous; no scope pointer is
published into HIR. Closure publication and paid capture constructors are
byte-identical, not redirected to a new funding or ownership policy.

The empty root is an explicit storage tradeoff: the Cow carrier may request
more bytes than the original empty-map Rc. The physical observer compares empty,
shallow and large maps without credits, normalization or excluded clone traffic,
and receives and drops complete current/original HIR and Diagnostics inside
observation. The map detachment test retains every Binding field, including
an actual separately allocated traversal-row Box. Frame size remains592.
No universal allocation reduction is claimed. No decrease is claimed for the
outstanding measured573330 resolution deficit.

`borrowed_root_oracle` retains34 complete frozen source files and190 complete
function bodies with independent old map/frame/scheduling/record-pattern/statement/mutation and closure
body receivers. Its wrapper borrows unchanged declaration/type services. The
PathArena API and traversal Row schema intentionally remain shared unchanged
ABI types; complete source and Row/Binding declaration proof makes this boundary
explicit. Existing recursive, saved capacity, fixed-result and paid closure
controls remain. Duplicate frozen test modules are disabled; their original
live owners and complete fixture sources are retained. The frozen nonrecursive
single-function receiver gets one fixed3089-line test-only exception; production
exceptions and Frame592/capacity limits are unchanged.

`source-provenance.py` is the lightweight owning source-proof producer. It checks
all complete frozen function bodies before producing hashes, verifies complete
unchanged shared/publication services and Binding/Row declarations, retains all
old fixture literals in order, and checks sizes for every changed Rust source.
Generated source proofs are not handwritten catalog/diagnostic pins. The new
joined-source library test reads every frozen helper/frame and unchanged shared
boundary. No production source was relocated or omitted from a text audit.

Qualification selectors: `selectors.json` / `qualification-selectors.txt`;
all are exact owning library or existing module_size/source_locked_contracts
binary names; use `-- --exact`, never a broad prefix. Source-proof producer:
`python3 docs/evidence/borrowed-root-bindings/source-provenance.py`.
No new Cargo test binary, catalog producer or pin edits.

Claude retry: actual `claude-sonnet-5-5`,80.85s,345473024 bytes peak owned
process-group RSS, tools/execution disabled, list cost$0.1947035. No concrete
blockers in visible source. Its conditional cfg/import/pointer/trait/census
assumptions were source-confirmed; unnecessary pointer-cast parentheses were
removed. The first request timed out180s with no model output; its cost/model
completion is unknown. Raw prompts/results/process receipts are retained.

Only rustfmt syntax formatting, diff inspection and the static source producer
ran here. No Cargo, compiler, build, test or full gate ran. Runtime allocation,
semantic/canonical/cache/loan behavior and actual residual improvement remain
pending the root's focused qualification. No root checkout or target was edited.
