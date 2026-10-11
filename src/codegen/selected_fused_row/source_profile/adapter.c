/* Compiler-owned native source-profile envelope; no program IO capability.
   Every managed request is prepaid before program effects. The subordinate
   structural ledger preserves its original boundary debits; it is not a second
   grant. Payload/String prepayment is cumulative, never reset or refunded. */
#include <inttypes.h>
#include <stdio.h>
_Static_assert(UINTPTR_MAX==UINT64_MAX,"source profile requires native64");
typedef struct {
    size_t limit, debited;
    uint32_t sealed, failed;
    spx_fused_issuer issuer;
    spx_fused_ledger structure;
    spx_fused_payload payload;
} spx_profile_account;
static __attribute__((unused)) int spx_profile_account_init(spx_profile_account *a,size_t limit) {
    if (!a || a->sealed || !limit) return 0;
    a->limit=limit;
    if (!spx_fused_issuer_init(&a->issuer) || !spx_fused_ledger_init(&a->structure,limit) ||
        !spx_fused_payload_init(&a->payload)) return 0;
    a->sealed=1;return 1;
}
static __attribute__((unused)) int spx_profile_invoke(spx_profile_account *a,
    struct spx_context *ctx,int64_t *result,spx_status_token *status) {
    if (!a || !a->sealed || a->failed) return SPX_FUSED_REFUSED;
    if (!ctx || !result || !status) { a->failed=1;return SPX_FUSED_REFUSED; }
    if (a->debited>a->limit || SPX_PROFILE_REQUIRED>a->limit-a->debited) {
        a->failed=1;return SPX_FUSED_REFUSED;
    }
    a->debited+=SPX_PROFILE_REQUIRED;
    int outcome=spx_fused_selected_entry_v1(ctx,&a->issuer,&a->structure,&a->payload,result,status);
    if (outcome==SPX_FUSED_REFUSED || outcome==SPX_FUSED_ABANDONED) a->failed=1;
    return outcome;
}
static void spx_profile_json_string(const char *s) {
    fputc('"',stdout);
    for (const unsigned char *p=(const unsigned char *)s;*p;p++) {
        if (*p=='"' || *p=='\\') { fputc('\\',stdout);fputc(*p,stdout); }
        else if (*p<32u) fprintf(stdout,"\\u%04x",(unsigned)*p);
        else fputc(*p,stdout);
    }
    fputc('"',stdout);
}
int main(void) {
    spx_profile_account account={0};
    struct spx_status_entry entries[1]={0};
    struct spx_context context;
    if (!spx_profile_account_init(&account,SPX_PROFILE_LIMIT) ||
        !spx_context_init(&context,UINT64_C(1),entries,UINT32_C(1),NULL,NULL,NULL)) return 70;
    int64_t result=INT64_MIN;
    spx_status_token token=SPX_STATUS_SUCCESS;
    int outcome=spx_profile_invoke(&account,&context,&result,&token);
    const struct spx_normalized_status *selected_status=NULL;
    if (outcome==SPX_FUSED_LANGUAGE_FAILURE) {
        selected_status=spx_status_resolve(&context,token);
        if (!selected_status) outcome=SPX_FUSED_ABANDONED;
    }
    printf("{\"schema\":\"semaprax.fused-source.v1\",\"profile\":\"fused-construction-v1\","
           "\"source_sha256\":\"%s\",\"target\":\"native\",\"account_limit\":%zu,"
           "\"managed_reserved\":%zu,",SPX_PROFILE_SOURCE_SHA256,account.limit,account.debited);
    int exit_code;
    if (outcome==SPX_FUSED_PUBLISHED) {
        printf("\"outcome\":\"returned\",\"result\":\"%" PRId64 "\"}\n",result);exit_code=0;
    } else if (outcome==SPX_FUSED_LANGUAGE_FAILURE) {
        const struct spx_normalized_status *status=selected_status;
        fputs("\"outcome\":\"language_failure\",\"status\":{\"domain\":",stdout);
        spx_profile_json_string(status->domain_id);
        printf(",\"code\":%u}}\n",(unsigned)status->code);exit_code=1;
    } else {
        printf("\"outcome\":\"administrative_refusal\",\"reason\":\"%s\"}\n",
            outcome==SPX_FUSED_REFUSED?"account_or_admission":"abandoned");exit_code=2;
    }
    return fflush(stdout)==0?exit_code:74;
}
