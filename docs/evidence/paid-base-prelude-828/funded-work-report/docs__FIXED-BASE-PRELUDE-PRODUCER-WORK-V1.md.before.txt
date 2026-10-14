# Fixed Base Prelude Producer Work v1

Status: explicit private proposal; ROOT contract review and owning execution required before activation. This is a bounded compiler-internal constructor, not a replay account, source admission route or application feature. Paid Invocation Base Prelude v1 remains unregistered. No byte/work ceiling, current corrected P, graph schema or public diagnostic boundary changes.

## Existing owner and bounded subject

The existing declaration constructor owns a raw byte callback. FunctionResolutionWork describes body reuse and cannot pay for declaration work. No unconditional successful work adapter or unrelated replay allowance is permitted. The proposed FixedSchemaPayer borrows that exact byte callback mutably and owns an inline finite counter for constructing only the immutable base schema: two variants and nine descriptors, unconditionally. Its constructor precharges actual Self initializer, Result<Self,E> receiver and WorkReceipt layouts on the existing byte owner before publishing the counter. The counter allocates no heap, captures no ambient ledger and holds no authored source or derived admission fact.

There is no public quota setter, generic work credit, replay lease, cap increase or source-name/use filter. Test-only restriction may reduce the fixed quota; it cannot raise it. Callers must retain the same counter through construction and refusal. Constructing another invocation is not a retry on a failed counter, and the counter cannot fund downstream source/replay work or reset their owners.

## Exact source-derived operation bound

The bound is computed by const evaluation over the existing base_schema::BASE metadata. The census creates no runtime duplicate AST/HIR or unpriced scratch. It reproduces byte ordering over immutable UTF-8 IDs and the producer's insertion-sort sequence. Compiler-owned duplicate IDs or undeclared field parameters fail the const model; this adds no user-source acceptance rule.

Let D be the ordered descriptor rows from BASE (type, then its cases and their fields). Let lookup(x) sum len(d.id)+len(x) in the sorted descriptor array through its exact matching row. The existing producer's logical work-site bound is:

```text
1
+ sum over D: len(id) + 2*len(name)
+ sum over actual insertion-sort comparisons: len(left.id)+len(right.id)
+ sum over types:
    lookup(type.id) + 2*len(type.name)
  + sum over parameters: 1 + 2*len(parameter.name)
  + sum over cases:
      lookup(case.id) + 2*len(case.name)
    + sum over fields:
        lookup(field.id) + 2*len(field.name)
      + sum over parameter-name comparisons through its match:
          len(parameter.name)+len(field.parameter)
```

These are the producer's already explicit logical units for ID copies, name copy/UTF-8 validation, parameter nodes and ordered comparison bounds. They are not CPU cycles, allocator time, process RSS or performance evidence. The fixed schema additionally bounds all surrounding scalar loops and constant Arc operations; no input-dependent recursive nominal walk is admitted. Base resolved field types remain TypeParameter leaves. No fresh arbitrary multiplier, observed gap or hand-adjusted numeric literal is introduced. A change to producer work must independently preserve/update this source census; the full constructor control requires actual consumed units to equal the derived bound.

## Sticky order and refusal

Before work, a zero-byte request authenticates the exact existing byte owner and preserves its already selected error E. New work cannot outrank a prior byte refusal. A work request exceeding the remaining fixed bound records its exact requested/remaining receipt once, returns a small typed Work error and spends nothing beyond the limit. All later positive/zero work and byte requests on that counter return the same first work refusal without consulting a foreign/new owner. A byte callback error is wrapped without formatting or allocating and remains the original E; the callback's existing sticky zero-request contract selects it on later attempts. No report is rendered inside this private counter.

The finished bundle remains independently owned and immutable. The fixed counter does not finance a new Index/HIR receiving carrier beyond its actual precharges, erase spent bytes or authorize publication after refusal. The root caller must preserve ordinary verification/base-source authentication and its existing funded outward refusal boundary. The new work error remains private; public activation/report adaptation is not claimed here.

## Authored controls and limits of evidence

The actual complete base producer runs against this counter, then its receipt must show the derived exact limit consumed, zero remaining and no refusal. A following one-unit request fails, and zero/larger requests plus whole-constructor retry retain the same receipt without new ownership. Reduced one-short and zero work limits exercise partial-constructor Drop under the same owner. Counter setup exact/one-short/zero bytes are independent Layout expectations; a byte refusal then wins even against an overflowing work request, retains the exact old byte error and creates no work receipt. Whole constructor/allocation/highwater/final Drop observations remain separate from logical work counters.

These are authored source controls only. No Cargo/build/runtime or new Claude review executed this proposal. ROOT must review the contract and execute the integrated owners before calling it funded activation. No generic work-fit, allocation saving, peak reduction, cache/backend parity or application improvement follows.
