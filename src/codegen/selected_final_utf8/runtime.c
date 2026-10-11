/* Selected native final UTF8 v1 only. Never included by default profiles. */
#include <stddef.h>
#define SPX_FINAL_UTF8_HANDLES 8u
/* Distinct-address mutable object; never a pooled literal/const-string identity. */
static unsigned char spx_final_utf8_module_identity;
_Static_assert(sizeof(void *)==8u,"selected final UTF8 requires native64");
struct spx_final_utf8_handle { const struct spx_final_utf8_lease *identity; uint32_t slot; uint64_t generation; };
struct spx_final_utf8_borrow { const struct spx_final_utf8_lease *identity; uint32_t slot; uint64_t generation; uint64_t epoch; };
struct spx_final_utf8_slot { char *text; uint64_t generation, borrow_epoch; bool reserved, borrowed; };
struct spx_final_utf8_lease {
    const struct spx_final_utf8_lease *identity;
    const void *module_identity;
    void *retained;
    struct spx_context *context;
    uint64_t context_generation, context_nonce;
    void *report_storage;
    uint64_t report_generation;
    bool report_live;
    void *(*allocate)(void *,size_t);
    void (*release)(void *,void *);
    void (*release_state)(void *);
    struct spx_final_utf8_slot slots[SPX_FINAL_UTF8_HANDLES];
    uint32_t active_slot;
    spx_status_token selected, allocation_refusal;
    bool used, live, busy;
};
struct spx_final_utf8_header { struct spx_final_utf8_lease *lease; uint64_t len; char data[]; };
_Static_assert(offsetof(struct spx_final_utf8_header,data)==16u,"selected String header layout");
static bool spx_final_utf8_valid_lease(const struct spx_final_utf8_lease *lease) {
    return lease!=NULL && lease->used && lease->live && lease->identity==lease && lease->module_identity==&spx_final_utf8_module_identity &&
        lease->retained!=NULL && lease->context!=NULL && lease->context->state==SPX_CONTEXT_INITIALIZED &&
        lease->context->generation==lease->context_generation && lease->context->invocation_nonce==lease->context_nonce && lease->allocate!=NULL && lease->release!=NULL && lease->release_state!=NULL;
}
/* Host retain owns allocator state AND exact module lifetime; invocation caller
 * separately pins executing code until this function and close have returned. */
bool semaprax_final_utf8_lease_init_v1(struct spx_final_utf8_lease *lease,void *state,
    void *(*retain)(void *),struct spx_context *(*held_context)(void *),void (*release_state)(void *),
    void *(*allocate)(void *,size_t),void (*release)(void *,void *)) {
    if(lease==NULL || retain==NULL || held_context==NULL || release_state==NULL || allocate==NULL || release==NULL ||
       lease->used || lease->live || lease->busy || lease->identity!=NULL || lease->module_identity!=NULL || lease->retained!=NULL ||
       lease->context!=NULL || lease->context_generation!=0 || lease->context_nonce!=0 || lease->report_storage!=NULL || lease->report_generation!=0 || lease->report_live || lease->allocate!=NULL || lease->release!=NULL || lease->release_state!=NULL || lease->selected!=0 || lease->allocation_refusal!=0 || lease->active_slot!=0) return false;
    for(unsigned i=0;i<SPX_FINAL_UTF8_HANDLES;++i) if(lease->slots[i].text!=NULL || lease->slots[i].generation!=0 ||
        lease->slots[i].borrow_epoch!=0 || lease->slots[i].reserved || lease->slots[i].borrowed) return false;
    /* Reserve ownership before calling external retain; reentry cannot init it. */
    lease->used=true;lease->busy=true;
    void *retained=retain(state);
    if(retained==NULL){lease->busy=false;return false;}
    struct spx_context *context=held_context(retained);
    if(context==NULL || context->state!=SPX_CONTEXT_INITIALIZED || context->call_depth!=0 || context->final_utf8_lease!=NULL || context->status_arena.entries==NULL || context->status_arena.length>=context->status_arena.capacity){release_state(retained);lease->busy=false;return false;}
    lease->context=context;lease->context_generation=context->generation;lease->context_nonce=context->invocation_nonce;
    lease->retained=retained;lease->allocate=allocate;lease->release=release;lease->release_state=release_state;
    lease->identity=lease;lease->module_identity=&spx_final_utf8_module_identity;lease->live=true;lease->busy=false;return true;
}
bool semaprax_final_utf8_lease_close_v1(struct spx_final_utf8_lease *lease) {
    if(!spx_final_utf8_valid_lease(lease) || lease->busy || lease->report_live || lease->report_storage!=NULL) return false;
    for(unsigned i=0;i<SPX_FINAL_UTF8_HANDLES;++i)if(lease->slots[i].reserved || lease->slots[i].text!=NULL || lease->slots[i].borrowed)return false;
    void *state=lease->retained;void (*release_state)(void *)=lease->release_state;
    lease->live=false;lease->identity=NULL;lease->module_identity=NULL;lease->retained=NULL;lease->context=NULL;lease->busy=true;
    release_state(state);lease->busy=false;return true;
}
static void spx_final_utf8_owned_drop(char *text) {
    if(text==NULL)spx_runtime_invariant_failure("null selected String drop");
    struct spx_final_utf8_header *header=(struct spx_final_utf8_header *)((uint8_t *)text-offsetof(struct spx_final_utf8_header,data));
    struct spx_final_utf8_lease *lease=header->lease;
    if(!spx_final_utf8_valid_lease(lease) || !lease->busy || lease->active_slot>=SPX_FINAL_UTF8_HANDLES)
        spx_runtime_invariant_failure("selected String has no active matching release");
    struct spx_final_utf8_slot *slot=&lease->slots[lease->active_slot];
    if(!slot->reserved || slot->text!=text || slot->borrowed)spx_runtime_invariant_failure("selected owner release mismatch");
    slot->text=NULL;lease->release(lease->retained,header);
}
static spx_status_token spx_final_utf8_failure(struct spx_context *ctx,const char *domain,uint32_t code) {
    struct spx_final_utf8_lease *lease=ctx->final_utf8_lease;
    if(lease->selected!=0)return lease->selected;
    if(!spx_status_record_adapter(ctx,domain,code,SPX_STATUS_CLASS_ADAPTER,SPX_RETRYABILITY_FALSE,&lease->selected))
        spx_runtime_invariant_failure("selected allocation status backing not reserved");
    return lease->selected;
}
static spx_status_token spx_final_utf8_construct(struct spx_context *ctx,const uint8_t *data,uint64_t length,char **out) {
    if(ctx==NULL || out==NULL || !spx_final_utf8_valid_lease(ctx->final_utf8_lease))
        spx_runtime_invariant_failure("missing selected allocator lease");
    struct spx_final_utf8_lease *lease=ctx->final_utf8_lease;
    if(!lease->busy || lease->active_slot>=SPX_FINAL_UTF8_HANDLES)spx_runtime_invariant_failure("selected constructor outside invocation");
    if(lease->selected!=0)return lease->selected;
    struct spx_final_utf8_slot *slot=&lease->slots[lease->active_slot];
    if(!slot->reserved || slot->text!=NULL || lease->allocation_refusal==0 || ctx->status_arena.entries==NULL || ctx->status_arena.length>=ctx->status_arena.capacity)
        spx_runtime_invariant_failure("selected constructor authority/backing not reserved");
    if(length>(uint64_t)SIZE_MAX-(uint64_t)offsetof(struct spx_final_utf8_header,data)-UINT64_C(1))
        return spx_final_utf8_failure(ctx,"semaprax.allocation.v1",UINT32_C(2));
    if(length>UINT64_C(131072))spx_runtime_invariant_failure("selected payload exceeds authenticated Bytes bound");
    if(length!=0 && data==NULL)spx_runtime_invariant_failure("invalid selected input slice");
    if(!spx_status_domain_is_utf8((const char *)data,(size_t)length))
        return spx_final_utf8_failure(ctx,"semaprax.convert.v1",UINT32_C(1));
    struct spx_final_utf8_header *header=lease->allocate(lease->retained,offsetof(struct spx_final_utf8_header,data)+(size_t)length+1u);
    if(header==NULL){
        if(lease->allocation_refusal==0 || spx_status_resolve(ctx,lease->allocation_refusal)==NULL)
            spx_runtime_invariant_failure("selected allocation backing was not recorded before callback");
        lease->selected=lease->allocation_refusal;return lease->selected;
    }
    header->lease=lease;header->len=length;if(length!=0)memcpy(header->data,data,(size_t)length);header->data[length]='\0';
    slot->text=header->data;*out=header->data;return SPX_STATUS_SUCCESS;
}
bool semaprax_final_utf8_borrow_v1(struct spx_final_utf8_lease *lease,struct spx_final_utf8_handle handle,
    struct spx_final_utf8_borrow *borrow,const uint8_t **data,uint64_t *length) {
    if(!spx_final_utf8_valid_lease(lease)||lease->busy||handle.identity!=lease||handle.slot>=SPX_FINAL_UTF8_HANDLES||borrow==NULL||data==NULL||length==NULL)return false;
    struct spx_final_utf8_slot *slot=&lease->slots[handle.slot];
    if(!slot->reserved||slot->text==NULL||slot->generation!=handle.generation||slot->borrowed||slot->borrow_epoch==UINT64_MAX)return false;
    ++slot->borrow_epoch;slot->borrowed=true;
    *borrow=(struct spx_final_utf8_borrow){lease,handle.slot,handle.generation,slot->borrow_epoch};
    *data=(const uint8_t *)slot->text;*length=spx_string_length_v10(slot->text);return true;
}
bool semaprax_final_utf8_end_borrow_v1(struct spx_final_utf8_lease *lease,struct spx_final_utf8_borrow borrow) {
    if(!spx_final_utf8_valid_lease(lease)||lease->busy||borrow.identity!=lease||borrow.slot>=SPX_FINAL_UTF8_HANDLES)return false;
    struct spx_final_utf8_slot *slot=&lease->slots[borrow.slot];
    if(!slot->reserved||slot->text==NULL||slot->generation!=borrow.generation||!slot->borrowed||slot->borrow_epoch!=borrow.epoch)return false;
    slot->borrowed=false;return true;
}
bool semaprax_final_utf8_drop_v1(struct spx_final_utf8_lease *lease,struct spx_final_utf8_handle handle) {
    if(!spx_final_utf8_valid_lease(lease)||lease->busy||handle.identity!=lease||handle.slot>=SPX_FINAL_UTF8_HANDLES)return false;
    struct spx_final_utf8_slot *slot=&lease->slots[handle.slot];
    if(!slot->reserved||slot->text==NULL||slot->generation!=handle.generation||slot->borrowed)return false;
    lease->busy=true;lease->active_slot=handle.slot;spx_final_utf8_owned_drop(slot->text);slot->reserved=false;lease->busy=false;return true;
}
