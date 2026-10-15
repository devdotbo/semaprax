//! Owning cross-source gate for the actual selected native C runtime.
//! Authored, unexecuted: exact repr(C) sizes, alignments and every field offset,
//! real administrative admission and original Vec receipt conversion, same
//! physical ledger across two invocations, no refund/reset, sticky short
//! account refusal and copied/foreign receipt fail-stop at O0 and O2.
//! This is an allocation-account receiving gate, not application acceptance
//! or an allocator measurement. The reference model is separate proof data.
use super::mirror;
use super::*;
use std::fmt::Write;

const RUNTIME: &str = include_str!("../../../../codegen/selected_fused_row/runtime.c");
const PLACEMENT: Placement = Placement {
    staged: 1,
    rows: 3,
    views: 1,
};

fn static_asserts(out: &mut String) {
    for (macro_name, value) in [
        ("SPX_FUSED_ROWS", u64::from(MAX_ROWS)),
        ("SPX_FUSED_VIEWS", VIEWS as u64),
        ("SPX_FUSED_EXTENT", MAX_EXTENT),
        (
            "SPX_FUSED_EVENTS",
            u64::from(crate::byte_data_capacity::MAX_BYTES_COPY_SITES),
        ),
        (
            "SPX_FUSED_FRAME_LIMIT",
            (MAX_PLACEMENT + mirror::DOMAIN_BYTES) as u64,
        ),
        (
            "SPX_FUSED_CUMULATIVE",
            crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES,
        ),
    ] {
        writeln!(
            out,
            "_Static_assert({macro_name} == {value}u, \"{macro_name} mirror\");"
        )
        .unwrap();
    }
    for shape in mirror::SHAPES {
        writeln!(
            out,
            "_Static_assert(sizeof({0}) == {1}u, \"{0} size\");\n\
             _Static_assert(_Alignof({0}) == {2}u, \"{0} alignment\");",
            shape.name, shape.size, shape.align
        )
        .unwrap();
    }
    for table in mirror::fields() {
        for field in table {
            writeln!(
                out,
                "_Static_assert(offsetof({0}, {1}) == {2}u, \"{0}.{1} offset\");\n\
                 _Static_assert(sizeof((({0} *)0)->{1}) == {3}u, \"{0}.{1} size\");",
                field.owner, field.name, field.offset, field.size
            )
            .unwrap();
        }
    }
    writeln!(
        out,
        "_Static_assert({}u * sizeof(spx_fused_staged) + {}u * sizeof(spx_fused_rows) + \
         {}u * sizeof(spx_fused_view) == {}u, \"placement formula\");",
        PLACEMENT.staged,
        PLACEMENT.rows,
        PLACEMENT.views,
        PLACEMENT.bytes().unwrap()
    )
    .unwrap();
}

/// The actual funded runtime, with shared accounts across invocations.
const ACTIVATION: &str = r#"
static const uint32_t spx_gate_order[3] = {SPX_FUSED_SCALAR, SPX_FUSED_STRING, SPX_FUSED_PAYLOAD};
SPX_FUSED_FN void spx_gate_string_drop(char *value) { (void)value; }
SPX_FUSED_FN int spx_gate_open(spx_fused_domain **out, spx_fused_issuer *issuer,
 spx_fused_ledger *ledger, spx_fused_payload *payload, size_t count, size_t frame) {
    void *storage=spx_fused_admit(issuer,ledger,payload,count,3u,frame);
    if (!storage) return 0;
    *out=storage;spx_fused_bind(*out,issuer,ledger,payload,count,3u,frame);return 1;
}
SPX_FUSED_FN int spx_gate_activation(size_t count,size_t frame,size_t metadata,size_t row_bytes) {
    spx_fused_issuer issuer={0};spx_fused_ledger short_account={0},ledger={0};
    spx_fused_payload short_payload={0},payload={0};spx_fused_domain *d=NULL;
    size_t total=metadata+row_bytes;
    if (!spx_fused_issuer_init(&issuer) || spx_fused_metadata_bytes(count,frame)!=metadata ||
        spx_fused_invocation_bytes(count,frame)!=total) return 10;
    if (!spx_fused_ledger_init(&short_account,total-1u) || !spx_fused_payload_init(&short_payload)) return 11;
    /* Administrative preflight refuses before any program effects. Sticky
       refusal debits neither account and does not mint a domain. */
    uint64_t before_nonce=issuer.next;
    if (spx_gate_open(&d,&issuer,&short_account,&short_payload,count,frame) ||
        !short_account.failed || short_account.debited || short_account.held_rows ||
        short_payload.events || short_payload.held_events || issuer.next!=before_nonce || issuer.active) return 12;
    if (spx_gate_open(&d,&issuer,&short_account,&short_payload,count,frame)) return 13;
    if (!spx_fused_ledger_init(&ledger,total*2u) || !spx_fused_payload_init(&payload)) return 14;
    uint64_t previous=0;
    for (unsigned run=0;run<2u;run++) {
        size_t before=(size_t)run*total;
        if (!spx_gate_open(&d,&issuer,&ledger,&payload,count,frame) ||
            ledger.debited!=before+metadata || ledger.held_rows!=row_bytes ||
            d->invocation<=previous) return 15;
        previous=d->invocation;
        /* An overlapping admission may not debit the same account. */
        spx_fused_domain *overlap=NULL;
        if (spx_gate_open(&overlap,&issuer,&ledger,&payload,count,frame) ||
            ledger.debited!=before+metadata || ledger.held_rows!=row_bytes) return 16;
        spx_fused_row_receipt *receipt=NULL;spx_fused_rows rows={0};
        spx_fused_rows_reserve(d,count,&receipt);
        if (receipt!=&d->rows_receipt || ledger.debited!=before+total || ledger.held_rows ||
            receipt->bytes!=row_bytes || receipt->stride!=sizeof(spx_fused_row)) return 17;
        if (!spx_fused_rows_allocate_funded(d,receipt,count,&rows) || receipt->pointer!=rows.rows ||
            ledger.debited!=before+total) return 18;
        spx_fused_row_receipt copied=*receipt;
        if (spx_fused_rows_receipt_current(d,&copied)) return 19;
        spx_fused_activate_with_rows(d,receipt);
        if (!d->slots || !d->views || !d->backing || ledger.debited!=before+total ||
            payload.events!=run+1u || payload.cumulative!=(size_t)(run+1u)*count*3u) return 20;
        /* Empty rows suffice for this physical receiving gate: no language
           finalizer or accepted-application result is invented. */
        spx_fused_abort(d);
        if (!spx_fused_rows_drop(d,&rows,spx_gate_order,spx_gate_string_drop) ||
            !spx_fused_settle_abort(d) || !spx_fused_finish(d)) return 21;
        if (!receipt->settled || ledger.debited!=before+total || ledger.held_rows || issuer.active) return 22;
        if (spx_fused_rows_receipt_settle(d,receipt)) return 23;
        free(d);d=NULL;
    }
    before_nonce=issuer.next;
    if (spx_gate_open(&d,&issuer,&ledger,&payload,count,frame) || !ledger.failed ||
        ledger.debited!=total*2u || ledger.held_rows || issuer.next!=before_nonce) return 24;
    return 0;
}
SPX_FUSED_FN void spx_gate_foreign_receipt(const char *mode) {
    size_t count=33u,frame=sizeof(spx_fused_domain),total=spx_fused_invocation_bytes(count,frame);
    spx_fused_issuer issuer={0},foreign={0};spx_fused_ledger ledger={0};spx_fused_payload payload={0};
    spx_fused_domain *d=NULL;spx_fused_rows rows={0};spx_fused_row_receipt *receipt=NULL;
    if (!spx_fused_issuer_init(&issuer) || !spx_fused_issuer_init(&foreign) ||
        !spx_fused_ledger_init(&ledger,total) || !spx_fused_payload_init(&payload) ||
        !spx_gate_open(&d,&issuer,&ledger,&payload,count,frame)) exit(80);
    spx_fused_rows_reserve(d,count,&receipt);
    if (!spx_fused_rows_allocate_funded(d,receipt,count,&rows)) exit(81);
    spx_fused_row_receipt copied=*receipt;
    if (mode[0]=='c') spx_fused_activate_with_rows(d,&copied);
    else { receipt->issuer=&foreign;spx_fused_activate_with_rows(d,receipt); }
    exit(82); /* A forged receipt must hit the real fail-stop receiver. */
}
"#;

fn probe() -> String {
    let mut out =
        String::from("#include <stdlib.h>\n#define SPX_FUSED_FAIL_STOP(message) exit(86)\n");
    out.push_str(RUNTIME);
    out.push('\n');
    static_asserts(&mut out);
    out.push_str(ACTIVATION);
    let placement = PLACEMENT.bytes().unwrap();
    let frame = mirror::DOMAIN_BYTES + placement;
    out.push_str("int main(int argc,char **argv) {\n    if(argc>1) spx_gate_foreign_receipt(argv[1]);\n    int status;\n");
    for (rows, slot) in [(33usize, 1), (4096, 2)] {
        let metadata = metadata_bytes(rows, placement).unwrap();
        let row_bytes = rows * mirror::ROW_BYTES;
        writeln!(out, "    if ((status = spx_gate_activation({rows}u, {frame}u, {metadata}u, {row_bytes}u))) return status + {offset};", offset = slot * 100).unwrap();
    }
    for (rows, placement) in [(0usize, 0usize), (4097, 0), (33, MAX_PLACEMENT + 1)] {
        assert_eq!(metadata_bytes(rows, placement), None);
        let frame = mirror::DOMAIN_BYTES + placement;
        writeln!(
            out,
            "    if (spx_fused_metadata_bytes({rows}u, {frame}u)) return 1;"
        )
        .unwrap();
    }
    let frame = mirror::DOMAIN_BYTES + MAX_PLACEMENT;
    writeln!(
        out,
        "    if (spx_fused_metadata_bytes(4096u, {frame}u) != {}u) return 2;\n    return 0;\n}}",
        metadata_bytes(4096, MAX_PLACEMENT).unwrap()
    )
    .unwrap();
    out
}

fn compile_and_run(name: &str, text: &str) {
    use std::process::Command;
    let root = std::env::temp_dir().join(format!(
        "fused-selected-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join(format!("{name}.c"));
    std::fs::write(&source, text).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "{name}{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        for hostile in ["copied", "foreign"] {
            let refusal = Command::new(&binary).arg(hostile).output().unwrap();
            assert_eq!(
                refusal.status.code(),
                Some(86),
                "actual receiving gate: {hostile}"
            );
        }
        let run = Command::new(&binary).output().unwrap();
        assert!(
            run.status.success(),
            "status {:?}: {}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

/// One compiled receiver gate covers both target layout and destination
/// funding, avoiding duplicate executions of the same complete C probe.
#[test]
fn selected_mirror_and_destination_funding_match_actual_c_runtime_on_one_ledger() {
    compile_and_run("layout-and-rows", &probe());
}

#[test]
fn generated_probe_covers_every_mirrored_field_once() {
    let text = probe();
    let mut fields = 0;
    for table in mirror::fields() {
        for field in table {
            let needle = format!("offsetof({}, {})", field.owner, field.name);
            assert_eq!(text.matches(&needle).count(), 1, "{needle}");
            fields += 1;
        }
    }
    assert_eq!(
        fields,
        4 + 5 + 6 + 2 + 11 + 4 + 3 + 7 + 5 + 4 + 3 + 3 + 7 + 25
    );
    for shape in mirror::SHAPES {
        assert!(text.contains(&format!("sizeof({}) ==", shape.name)));
    }
}
