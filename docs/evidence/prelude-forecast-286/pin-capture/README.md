# Source-only pin capture plan

No producer or compiled gate ran in this lane. No pin literal is changed here.
The exact existing owners and separate ignored producer are registered in
`selectors.json`. Their original assertions remain active. There was no existing
environment regeneration support; the two owning integration tests now accept
an optional `SEMAPRAX_PRELUDE_PIN_CAPTURE_DIR` evidence sink. It creates fresh
files from actual API outputs before existing literal assertions and never
replaces prior evidence. The existing ignored library producer still prints its
full actual projection wire with its original marker.

## Root execution after the focused build

Use already-built **test executables**, from the same root source build. Do not
pass the CLI executable. No additional Cargo target is needed for the initial
capture. Run from the integrated root checkout:

```sh
scripts/capture-prelude-forecast-pins.sh \
  /absolute/private-target/debug/deps/semaprax-TEST_HASH \
  /absolute/private-target/debug/deps/workspace-TEST_HASH \
  /absolute/fresh-evidence-directory
```

The script runs precisely the two existing public graph/analysis test owners
plus the existing ignored full-wire library producer, serially. It writes raw
logs, each exact selector's exit status, source head/local patch, binary hashes,
full actual wire bytes, and `candidates.json`. Ordinary failing assertions remain
failures in `status.tsv`; capture success is not gate success. Missing actual
artifacts produce script exit 3. The script never edits source pins, guesses a
value, builds, selects another test binary, or runs a full gate.

## Review and update from actual bytes only

1. Preserve the original test literals and complete historical receipt/source
   files here. The two pre-hook test sources have exact inverse records; the
   browser JS owner is also archived byte-for-byte. Retain this evidence when
   updating current pins; do not normalize historical values.
2. Check source/binary provenance and every owner log. Read actual full graph
   JSON and budget before taking its numeric debit and document SHA. Read all
   eight full analysis wires before taking their document SHAs; inspect their
   embedded graph digest bindings. The projection producer emits its complete
   JSON, whose document SHA is computed directly from those captured bytes.
3. Compare full historical/current wires where available. For this correction,
   audit the forecast-derived builder debit and authenticated digest changes;
   reject unrelated field drift. The script does not assert exclusive causality
   or manufacture a historical wire from a new forecast.
4. Update only reviewed current literals in the owning tests from
   `candidates.json` and actual JSON bytes. Preserve old literals/history in the
   archives. Rerun those same exact owners to qualify the new pins. A failed or
   absent producer output is not a candidate.

The Wasm browser owner is deferred for the first capture and introduces no new
required build target. Its baseline/renamed graph-digest pins are affected by a
changed builder budget. When root elects to run that existing owner, append its
already-built Wasm test executable as the optional fourth script argument. Its
original baseline assertion can stop before the renamed producer executes;
review/update only captured baseline literals, then rerun into another fresh
evidence directory to obtain the actual renamed graph. The original assertions
are not bypassed to accelerate capture. The browser fixture JS pins must come
from those actual baseline/renamed producer outputs, followed by its existing
qualification selector. Project/workspace revision pins must not move without
actual source-evidence justification.

Alternative existing real browser artifact producers, when independently
needed, are the CI commands in `.github/workflows/ci.yml`:

```sh
cargo run --locked -p semaprax -- build examples/calculator-project/semaprax.toml \
  --target web -o /absolute/isolated-baseline/generated
cargo run --locked -p semaprax -- build /absolute/isolated-renamed-project/semaprax.toml \
  --target web -o /absolute/isolated-renamed/generated
```

The renamed input is a separate copied calculator project with exactly the
existing `fn add` → `fn sum` display rename. These commands are documented only;
this packet does not run them or expand the current focused qualification.
