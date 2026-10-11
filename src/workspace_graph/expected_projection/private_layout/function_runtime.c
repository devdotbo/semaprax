int main(void) {
 struct spx_status_entry entries[32]; struct spx_context context={0};
 if(!spx_context_init(&context,UINT64_C(17),entries,32,NULL,NULL,NULL))return 1;
 for(unsigned repeat=0;repeat<3;++repeat){
  int64_t value=INT64_C(123456789);
  spx_status_token token=spx_decl_6170702e6d61696e(&context,&value);
  if(/* expected failure */){
   if(token==SPX_STATUS_SUCCESS||value!=INT64_C(123456789))return 2;
   const struct spx_normalized_status*status=spx_status_resolve(&context,token);
   if(!status||strcmp(status->domain,"semaprax.contract.v1")||status->code!=UINT32_C(2))return 3;
  }else if(token!=SPX_STATUS_SUCCESS||value!=INT64_C(42))return 4;
  if(layout_live||context.call_depth)return 5;
#ifdef SPX_VEC_AUTHORITY_CAPACITY
  for(uint32_t i=0;i<SPX_VEC_AUTHORITY_CAPACITY;++i)if(context.vec_authority[i].live)return 6;
#endif
 }
 return 0;
}
