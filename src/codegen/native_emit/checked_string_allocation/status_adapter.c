/* Private kernel-only u32 adapter precursor. No SEMAPRAX source/profile selects
 * this asset and it supplies no expression failure or cleanup authority. */
#ifndef SPX_CHECKED_STRING_KERNEL_ONLY_V1
#error "checked String adapter requires explicit private kernel-only selection"
#endif
#include <stdlib.h>
struct spx_checked_string_adapter_v1 {
    struct spx_context *semantic;
    struct spx_checked_string_context_v1 kernel;
    spx_status_token selected;
};
static bool spx_checked_string_adapter_init_v1(
    struct spx_checked_string_adapter_v1 *adapter, struct spx_context *semantic,
    void *state, void *(*allocate)(void *, size_t), void (*release)(void *, void *)
) {
    if (adapter == NULL || semantic == NULL || allocate == NULL || release == NULL ||
        adapter->semantic != NULL || adapter->kernel.allocate != NULL ||
        adapter->kernel.release != NULL || adapter->kernel.allocator_state != NULL ||
        adapter->selected != SPX_STATUS_SUCCESS || adapter->kernel.selected.code != 0 ||
        adapter->kernel.selected.schema != NULL || adapter->kernel.selected.domain_id != NULL ||
        adapter->kernel.selected.class_name != NULL || adapter->kernel.selected.retryable ||
        semantic->state != SPX_CONTEXT_INITIALIZED ||
        semantic->status_arena.entries == NULL ||
        semantic->status_arena.length >= semantic->status_arena.capacity) return false;
    *adapter=(struct spx_checked_string_adapter_v1){
        .semantic=semantic,
        .kernel={.allocator_state=state,.allocate=allocate,.release=release},
        .selected=SPX_STATUS_SUCCESS
    };
    return true;
}
static spx_status_token spx_checked_string_adapter_failure_v1(
    struct spx_checked_string_adapter_v1 *adapter
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    const struct spx_checked_string_status_v1 *failure=&adapter->kernel.selected;
    if (failure->schema == NULL || failure->domain_id == NULL || failure->class_name == NULL) abort();
    bool allocation=strcmp(failure->domain_id,"semaprax.allocation.v1")==0 &&
        (failure->code==UINT32_C(1) || failure->code==UINT32_C(2));
    bool conversion=strcmp(failure->domain_id,"semaprax.convert.v1")==0 &&
        failure->code==UINT32_C(1);
    if (strcmp(failure->schema,SPX_STATUS_SCHEMA_V1)!=0 ||
        strcmp(failure->class_name,"adapter")!=0 || failure->retryable ||
        !(allocation || conversion)) abort();
    spx_status_token token=SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(adapter->semantic,failure->domain_id,failure->code,
        SPX_STATUS_CLASS_ADAPTER,SPX_RETRYABILITY_FALSE,&token)) abort();
    adapter->selected=token;
    return token;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_from_literal_v1(
    struct spx_checked_string_adapter_v1 *adapter, const char *input, uint64_t length, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_from_literal_v1(&adapter->kernel,input,length,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_clone_v1(
    struct spx_checked_string_adapter_v1 *adapter, const char *input, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_clone_v1(&adapter->kernel,input,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_from_char_v1(
    struct spx_checked_string_adapter_v1 *adapter, uint32_t scalar, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_from_char_v1(&adapter->kernel,scalar,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_from_i64_v1(
    struct spx_checked_string_adapter_v1 *adapter, int64_t number, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_from_i64_v1(&adapter->kernel,number,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_from_usize_v1(
    struct spx_checked_string_adapter_v1 *adapter, uint64_t number, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_from_usize_v1(&adapter->kernel,number,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_from_str_v1(
    struct spx_checked_string_adapter_v1 *adapter, const char *input, uint64_t length, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_from_str_v1(&adapter->kernel,input,length,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_from_utf8_v1(
    struct spx_checked_string_adapter_v1 *adapter, const uint8_t *input, uint64_t length, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_from_utf8_v1(&adapter->kernel,input,length,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_concat_v1(
    struct spx_checked_string_adapter_v1 *adapter, const char *left, const char *right, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_concat_v1(&adapter->kernel,left,right,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_checked_string_adapter_collection_clone_v1(
    struct spx_checked_string_adapter_v1 *adapter, const char *input, char **out
) {
    if (adapter->selected != SPX_STATUS_SUCCESS) return adapter->selected;
    if (out == NULL) abort();
    char *candidate=NULL;
    if (!spx_checked_string_collection_clone_v1(&adapter->kernel,input,&candidate))
        return spx_checked_string_adapter_failure_v1(adapter);
    *out=candidate;
    return SPX_STATUS_SUCCESS;
}
