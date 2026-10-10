use super::super::{
    native_emit::{
        c_case_symbol, c_field_symbol, c_function_symbol, c_record_symbol, c_variant_symbol,
    },
    COutput,
};
use super::*;
use crate::variant_layout::{VariantLayout, VariantTarget};
pub(in crate::codegen) fn emit_adapter(
    output: &mut impl COutput,
    program: &ResolvedProgram,
    selection: &Selection,
) -> Result<(), Diagnostic> {
    let root = program
        .functions
        .iter()
        .find(|f| f.id == selection.entry)
        .ok_or_else(|| error("selected root missing during adapter emission"))?;
    let layout = VariantLayout::for_type(program, VariantTarget::Native64, &root.return_type)?;
    let encoded = layout
        .case(&selection.encoded_case)
        .ok_or_else(|| error("selected String case missing"))?;
    let refused = layout
        .case(&selection.refused_case)
        .ok_or_else(|| error("selected refusal case missing"))?;
    let input = c_record_symbol(&root.params[0].ty);
    let result = c_variant_symbol(&root.return_type);
    let entry = c_function_symbol(&root.id);
    let enc = c_case_symbol(&selection.encoded_case);
    let text = c_field_symbol(&selection.text_field);
    let no = c_case_symbol(&selection.refused_case);
    let required = c_field_symbol(&selection.required_field);
    let factory = program
        .functions
        .iter()
        .find(|f| f.id == selection.factory)
        .ok_or_else(|| error("checked Report factory missing during emission"))?;
    let factory_symbol = c_function_symbol(&factory.id);
    let dispose = c_function_symbol(&selection.disposer);
    let factory_args = if factory.params.is_empty() {
        ""
    } else {
        "factory_input,"
    };
    writeln!(output,r#"
/* Input provenance is minted by the checked source factory. Foreign Report
 * pointers and raw selected String values are never public adapter arguments. */
struct semaprax_final_utf8_report_v1 {{ const struct spx_final_utf8_lease *identity; uint64_t generation; }};
struct semaprax_final_utf8_result_v1 {{ uint32_t kind; struct spx_final_utf8_handle owner; uint64_t required; }};
static bool spx_final_utf8_ready(struct spx_final_utf8_lease *lease) {{
    return spx_final_utf8_valid_lease(lease) && !lease->busy && lease->context->call_depth==0 &&
        lease->context->final_utf8_lease==NULL && lease->context->status_arena.entries!=NULL &&
        lease->context->status_arena.length<=lease->context->status_arena.capacity &&
        lease->context->status_arena.capacity-lease->context->status_arena.length>=UINT32_C(2);
}}
static bool spx_final_utf8_report_matches(struct spx_final_utf8_lease *lease,struct semaprax_final_utf8_report_v1 report) {{
    return report.identity==lease && report.generation==lease->report_generation && lease->report_live && lease->report_storage!=NULL;
}}
bool semaprax_final_utf8_prepare_v1(struct spx_final_utf8_lease *lease,uint64_t factory_input,
    spx_status_token *status_out,struct semaprax_final_utf8_report_v1 *report_out) {{
    if(!spx_final_utf8_ready(lease) || status_out==NULL || report_out==NULL || lease->report_live ||
       lease->report_storage!=NULL || lease->report_generation==UINT64_MAX)return false;
    (void)factory_input;
    ++lease->report_generation;lease->busy=true;
    /* Fixed backing is reserved before any external allocator callback. Failure
     * here is a transport refusal, not an invented source allocation producer. */
    struct {input} *storage=lease->allocate(lease->retained,sizeof(struct {input}));
    if(storage==NULL){{lease->busy=false;return false;}}
    lease->report_storage=storage;
    spx_status_token status={factory_symbol}(lease->context,{factory_args}storage);
    if(status!=SPX_STATUS_SUCCESS){{
        lease->report_storage=NULL;lease->release(lease->retained,storage);lease->busy=false;*status_out=status;return true;
    }}
    lease->report_live=true;lease->busy=false;*status_out=SPX_STATUS_SUCCESS;
    *report_out=(struct semaprax_final_utf8_report_v1){{lease,lease->report_generation}};return true;
}}
bool semaprax_final_utf8_release_report_v1(struct spx_final_utf8_lease *lease,struct semaprax_final_utf8_report_v1 report,
    spx_status_token *status_out) {{
    if(!spx_final_utf8_ready(lease) || status_out==NULL || !spx_final_utf8_report_matches(lease,report))return false;
    lease->busy=true;lease->report_live=false;
    struct {input} *storage=lease->report_storage;int64_t ignored;
    /* The exact checked Own-Report constant-zero function performs default
     * Report settlement; the retained callback pair releases only its backing. */
    spx_status_token status={dispose}(lease->context,storage,&ignored);
    lease->report_storage=NULL;lease->release(lease->retained,storage);lease->busy=false;*status_out=status;return true;
}}
bool semaprax_final_utf8_invoke_v1(struct spx_final_utf8_lease *lease,struct semaprax_final_utf8_report_v1 report,
    uint64_t limit,spx_status_token *status_out,struct semaprax_final_utf8_result_v1 *result_out) {{
    if(!spx_final_utf8_ready(lease) || !spx_final_utf8_report_matches(lease,report) || status_out==NULL || result_out==NULL)return false;
    struct spx_context *ctx=lease->context;
    unsigned index=SPX_FINAL_UTF8_HANDLES;
    for(unsigned i=0;i<SPX_FINAL_UTF8_HANDLES;++i)if(!lease->slots[i].reserved && lease->slots[i].text==NULL &&
        !lease->slots[i].borrowed && lease->slots[i].generation!=UINT64_MAX){{index=i;break;}}
    if(index==SPX_FINAL_UTF8_HANDLES)return false;
    struct spx_final_utf8_slot *slot=&lease->slots[index];
    ++slot->generation;slot->reserved=true;
    lease->busy=true;lease->active_slot=index;lease->selected=SPX_STATUS_SUCCESS;ctx->final_utf8_lease=lease;
    /* A real immutable u32 arena entry is recorded before any source/host
     * allocator runs. It is selected only if the final allocator refuses;
     * the second free cell covers ordinary contract/conversion failure. */
    if(!spx_status_record_adapter(ctx,"semaprax.allocation.v1",UINT32_C(1),SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_FALSE,&lease->allocation_refusal))spx_runtime_invariant_failure("selected allocation backing reservation failed");
    struct {result} value;
    spx_status_token status={entry}(ctx,(struct {input} *)lease->report_storage,limit,&value);
    ctx->final_utf8_lease=NULL;
    if(status!=SPX_STATUS_SUCCESS){{
        if(slot->text!=NULL)spx_runtime_invariant_failure("selected failure retained a result owner");
        slot->reserved=false;lease->busy=false;*status_out=status;return true;
    }}
    struct semaprax_final_utf8_result_v1 published={{0}};
    if(value.spx_tag==UINT32_C({encoded_tag})){{
        if(slot->text==NULL || slot->text!=value.spx_payload.{enc}.{text})spx_runtime_invariant_failure("selected result owner differs from authenticated constructor");
        published.kind=UINT32_C(1);published.owner=(struct spx_final_utf8_handle){{lease,index,slot->generation}};
    }}else if(value.spx_tag==UINT32_C({refused_tag})){{
        if(slot->text!=NULL)spx_runtime_invariant_failure("refused result retained a selected owner");
        slot->reserved=false;published.kind=UINT32_C(2);published.required=value.spx_payload.{no}.{required};
    }}else spx_runtime_invariant_failure("selected outcome tag outside authenticated layout");
    lease->busy=false;*status_out=SPX_STATUS_SUCCESS;*result_out=published;return true;
}}
"#,encoded_tag=encoded.tag,refused_tag=refused.tag).expect("writing selected adapter cannot fail");
    Ok(())
}
