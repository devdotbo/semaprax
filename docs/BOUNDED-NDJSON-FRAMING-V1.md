# Bounded NDJSON framing v1

Status: source implementation and owning regression definitions authored; executable qualification pending. This is the bounded LF-only Catalog Normalizer composition tracked by [OPT #760](https://github.com/wavect/semaprax/issues/760). It does not change the LogLens line policy or the Catalog restock command.

## API and authority

`std.data.ndjson` is an ordinary checked source package depending on `std.io.lines` and `std.data.json.utf8`. It has no host imports, Web exports, borrowed-return exception, new byte carrier, or allocation intrinsic. Public declarations and fields have persistent IDs in `std/data-ndjson/src/ndjson.spx`.

`record_count`, `line_start`, and `line_end` observe an immutable borrowed Slice. `frame_at` returns a Copy `Frame { ordinal, start, end, next }`; `frame_matches` reconstructs those numeric cuts from the current body. A Frame is not a span certificate: another body with identical cuts can satisfy it. There is no owner token or generation in the record. These random-access helpers rescan the prefix/body; they are not a linear batch cursor. The adopted coordinator retains its own single count and monotonic cuts.

`basic_fault(body, frame)` first requires numeric agreement, then forms an ordinary `byte_range` in the receiving function. `basic_line_fault(line, ordinal)` receives an actual borrowed view, permitting a coordinator that already knows its current cuts to avoid repeated prefix reconstruction. Both routes retain independent source/HIR byte-range origin, extent, operation, identity and loan validation. Numeric facts grant no authority to move/drop an owner, forge a root, return a Slice, or retain it after settlement. The existing rules in [Portable indexed byte data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md) remain mandatory.

## Bounds and faults

The selected framing policy is a body of at most 65536 bytes, at most 256 records, and each line at most 512 bytes. LF separates records; CR remains content. Empty input and a lone LF have zero records. A final LF creates no additional record; interior empty lines fail. Frame intervals exclude LF, and `next` includes it when present. No UTF-8 or JSON normalization occurs during framing.

`Fault { category: i64, record_index: i64, byte_offset: usize }` is a Copy observation, not a language failure status. Category 0 means no framing fault, 1 a size/count limit, 2 malformed UTF-8, and 3 an empty record. Global faults use index -1 for the body and 256 for excess record count. Record indices are zero based and byte offsets are relative to the line. Whole-body size precedes record count; per-line size precedes UTF-8, which precedes empty-record rejection. UTF-8 offsets retain the existing earliest failing sequence-start convention. JSON grammar, field rules, decoded duplicate detection, checked accumulation and final publication remain in the consuming coordinator.

`line_end` requires a start within the current Slice; `frame_at` requires bounded body and a present ordinal; `basic_line_fault` requires ordinal below 256. The adopted legacy Catalog Normalizer wrappers retain their previously total behavior: out-of-range starts return the original start, and ordinals outside the new profile use the unchanged original fault computation. No strengthened legacy precondition is introduced.

## Composition and qualification

`examples/catalog-normalizer-project/src/batch.spx` forms the actual receiving range and maps structured faults back to its unchanged packed application error representation. Strict JSON/field ordering, decoded ID duplicate priority, checked totals and all-or-nothing publication remain unchanged. The frozen [Catalog Normalizer oracle v1](CATALOG-NORMALIZER-ORACLE-V1.md) still owns all application behavior and its 160M fuel ceiling. Catalog23 remains a separate regression.

Owning definitions are `standard_library::ndjson::{bounded_ndjson_framing_and_structured_faults_execute_on_all_three_backends, ndjson_receiving_range_rejects_foreign_root_type_operation_and_stale_producer, ndjson_forged_frame_contract_is_sticky_and_failed_ranges_never_publish}` in the project harness. They cover interpreter/C11 O0/O2/Core Wasm success, exact/one-over framing bounds, current-HIR root/type/operation/producer mutations, numeric overlap/out-of-range faults and interpreter sticky failed publication. Native/Wasm requires-false reentry and poisoned publication for the new Frame API remain a deferred qualification seam; normalizer backend serialized-fault coverage does not substitute for it. They do not claim an origin-bound cached frame receipt.

The useful-data harness retains `catalog_normalizer_application::batch_boundaries_and_string_normalization_agree_across_backends` and adds `catalog_normalizer_application::ndjson::old_framing_and_new_structured_receiver_preserve_the_complete_normalizer_oracle`. The latter uses the unchanged original batch source as an independent differential control and runs the complete published36/source14/max-output/backend matrix for both paths. Generated standard-library catalogs must be refreshed with the owning producer after integration. No backend equivalence, fit, performance gain, or full issue completion is established until these definitions and the unchanged ownership/storage gates execute on the integrated source.
