# Bounded independent prelude pin updater review

Status: completed source review of the original private updater; corrective work remains separate.
Audience: root capture, pin-review and qualification lanes.

One explicitly authorized tool-disabled Sonnet review inspected the exact updater SHA recorded in payload-whitelist.json, plus small capture/assertion/producer scopes pinned to root contract a184d4406f241aa11df5c297102609b4c35ec1b3. The updater snapshot equals private d5c5f366f1bfe2800839fd5aa9d99528280bc987. The actual capture callback did not exist and was not accessed; the updater was not run. No compiler/build, pin calculation from runtime, root source mutation or GitHub action occurred. Review artifacts alone are saved in this private evidence lane.

Returned model: claude-sonnet-5-5. One turn completed in103.86091679099991 seconds, exit0, sampled process-group peakRSS290373632 bytes. The120-second/768MiB/2MiB guards were not reached. Provider list costUSD0.169846; actual billing is unknown. No savings or automatic approval is claimed. Raw findings/result/receipt and exact source whitelist remain unchanged.

Confirmed findings in the reviewed draft:

- B1: graph_digest validation incorrectly used raw SHA(payload without graph_digest). The actual workspace producer uses its exact domain including terminal NUL, then u64 little-endian payload length, then payload. Document pin SHAs correctly hash the complete wire without that domain framing.
- B2: accepting every101 test exit with only selector/running-one evidence can accept an unrelated getter/schema/edge/locking assertion failure. Expected stale pin failures need exact attribution before any apply.
- B3/B4: capture self-consistency is not external provenance. The updater needs full approved source SHA and trusted ROOT binary/build/producer receipt linkage; the existing ROOT wrapper owns binary-to-source proof and clean-tree verification including untracked input. The standalone capture records only git diff and binary hashes, which do not establish those facts themselves.
- A1/A2/A3: validate every raw source byte snapshot before writing either file, preserve byte/newline identity in hashes and replacement, and reject evidence destinations inside capture before copying. The reviewed per-file text write loop can partially update before a later source-drift failure and can normalize CRLF.

Verified scope: exactly11literal edits are derived from actual complete wire bytes (one builder usage, one public graph SHA, eight analysis SHAs, one projection SHA). Old literals must match uniquely. No guessed hash or weakened assertion appears. The review supplies no actual candidate values or current callback authentication.

A separate bounded local source comparison verifies that private correction4276269fc2225c0c0fdc8aef839961e16b90006c adds only the exact workspace-artifact digest helper and routes internal graph validation through it. The complete reviewed source SHA, corrected SHA and exact patch are retained. All raw document SHA pin computations and all other checks remain unchanged in that correction. This is source resolution of B1, not a second Claude review, execution or validation of later fixes.

Root directed the prelude lane to resolve B2 through exact authorized pin-assertion attribution, B3/B4 through trusted external ROOT build/producer receipts/full40SHA/clean-source proof, and apply behavior through all-source prevalidation plus raw byte preservation and safe evidence placement. Those instructions and raw blockers were forwarded. No repeated Claude attempt is needed or claimed for that well-scoped deterministic work. Integrate this evidence only after the active root freeze permits it.
