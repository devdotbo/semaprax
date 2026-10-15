# Public Project Scaffold Capsule v8: current authoring guidance

Status: authored source; focused executable qualification is pending.

Audience: people and agents starting projects, and compiler contributors.

`semaprax project-scaffold --name <name> --layout current` renders table
manifests and corrected project guidance under `semaprax.project-scaffold.v8`.
Both standalone and full-toolchain `semaprax new` select this version. The
existing `frozen` and `tables` selectors retain their original v2–v7 capsules,
including their exact documentation bytes, hashes, and digest domains. Existing
CLI defaults for `project-scaffold` remain unchanged.

V8 derives the same template under the existing table-layout owner, preserves
all non-`AGENTS.md` files byte-for-byte, then updates only its guide. The guide
starts at bounded `help language author:routes`, identifies profile and target
selection, and names exact library prerequisite lookup. It distinguishes value
and statement `if`, describes condition-controlled `while`, and points to the
admitted lexical-exit profile rather than prohibiting all returns and exits.
It adds no syntax, capability, dependency, target support or publication right.
Existing complete source examples and template-specific dependency guides stay
intact. Profile restrictions remain ordinary compiler checks.

The capsule retains the closed eight-field root, template inventories, Project
schema identities and limits of the previous owners. The independent digest
domain is `semaprax.project-scaffold.digest.v8` followed by NUL, the little-endian
u64 length, and canonical descriptor bytes without digest. Replay recognizes
v8, rederives current guidance, and compares the complete canonical bytes and
digest. Changing a schema label or substituting a prior digest refuses with
`SPX-J115`; the capsule grants no filesystem or host authority.

Owning gates are `scaffold::current_guidance_capsule_preserves_legacy_sources_and_replays_all_templates`
and `scaffold_cli::public_cli_current_guidance_is_exact_replayable_and_inert`,
alongside all existing legacy scaffold, standalone creation and held-parent
publication controls. CLI parsing retains closed, case-sensitive values and
explicit duplicate-option refusal. No gates were executed when authoring this
version; earlier hosted evidence does not qualify v8.

The frozen guide intro is restored to its original pinned bytes. Commit
`1bd740386` had changed that paragraph in the v2–v7 producer and its literal
fixture without changing capsule versions or regenerating the original pins.
V8 alone publishes the updated bounded route guidance. The original v2 digest,
file hashes, full literal comparison, 5100-byte descriptor and replay assertions
remain unchanged; focused executable confirmation is pending.
