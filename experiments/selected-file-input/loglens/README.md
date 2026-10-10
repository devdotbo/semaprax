# Selected-file LogLens adoption

Status: authored native reference; execution and original CLI49 qualification pending.
Audience: compiler maintainers and LogLens application authors.

This new reference derives from the immutable accepted source at
`experiments/ascii-pattern-source/loglens-reference/qualified-source/`.
Historical source, provenance, SPEC, oracle, corpus and results remain unchanged.
Its retained acceptance does not qualify this adaptation or measure savings.

The application selects `source-command.selected-file.resource-output.v1`
(Project v34), input `argv-utf8-256+selected-file.follow.v1`, native64 Unix.
Only checked source validates filename/option usage. `run.sh` locates the
executable and prepends the fixed host selector for application argv index 0;
it never resolves, rewrites, normalizes or opens the input path or its options.
The native provider follows the explicit host-selected path on first checked
read and holds that descriptor until settlement. File bytes remain bounded at
65536, raw argv at 256 arguments/65536 aggregate bytes, selected spelling at
4096 bytes, and String/staged output at the inherited v28 1 MiB bounds.
Numeric byte counts have no machine-integer ceiling inside the bounded file;
these physical limits are not a new numeric range restriction.

Set `SEMAPRAX_BIN` to a compiler built from the joined source and run:

```sh
./build.sh
./run.sh ../../../benchmarks/cli-tokens-v1/sample.log --top 3 --json
./test.sh
```

`tests.mjs` retains every original test and assertion, including the
33,000-digit report, thirty repeated `--json` flags, option order and repeated
`--top`, usage before missing-file acquisition, leading-zero top values,
Unicode, line endings, malformed input, exact output bytes and exit statuses.
Only the public sample's repository-relative location changes. Source takes
over the prior shell's filename-flag and ASCII-digit checks; report parsing,
aggregation, sorting, exact decimal arithmetic and rendering remain unchanged.
The owning Project gate independently runs the unchanged historical33 plus
all16 SPEC boundary checks against both native O0/O2 builds. Extra selected
filename controls are additive and cannot replace any of those49 assertions.
No benchmark, matched agent campaign, production qualification or measured
source/token/cost improvement is claimed.
