/* Independent IEEE bit oracles, including NaN payloads, not generated expected
   output. Exercise the real C helper retained by checked F32/F64 source. */
{
 const uint64_t ordered64[8]={
  UINT64_C(0xfff8000000000002),UINT64_C(0xfff8000000000001),
  UINT64_C(0xfff0000000000000),UINT64_C(0x8000000000000000),
  UINT64_C(0),UINT64_C(0x7ff0000000000000),
  UINT64_C(0x7ff8000000000001),UINT64_C(0x7ff8000000000002)};
 const uint64_t ordered32[8]={
  UINT64_C(0xffc00002),UINT64_C(0xffc00001),UINT64_C(0xff800000),UINT64_C(0x80000000),
  UINT64_C(0),UINT64_C(0x7f800000),UINT64_C(0x7fc00001),UINT64_C(0x7fc00002)};
 const unsigned input[8]={4,7,1,6,3,0,5,2};
 const uint32_t key[1]={1},asc[1]={0},desc[1]={1};
 for(uint32_t code=6;code<=7;++code){
  const uint64_t *oracle=code==6?ordered32:ordered64;
  const uint32_t tags[2]={1,code};spx_vec_v1 v={0},next={0};
  if(spx_copy_vec_new(&c,2,8,&v)!=SPX_STATUS_SUCCESS)return 21;
  for(unsigned i=0;i<8;++i){uint64_t row[2]={input[i],oracle[input[i]]};
   if(spx_copy_vec_push(&c,&v,2,row,&next)!=SPX_STATUS_SUCCESS){return 22;}
   v=next;next=(spx_vec_v1){0};}
  void *storage=v.ptr;uint32_t authority=v.authority;uint64_t epoch=c.vec_next_generation;
  spx_sort_active=1;
  if(spx_copy_vec_sort_keys(&c,&v,2,tags,1,key,asc,&next)!=SPX_STATUS_SUCCESS)return 23;
  spx_sort_active=0;
  if(next.ptr!=storage||next.authority!=authority||next.generation!=epoch||c.vec_next_generation!=epoch+1||v.generation)return 24;
  uint64_t *words=(uint64_t*)(void*)next.ptr;
  for(unsigned i=0;i<8;++i)if(words[2*i]!=i||words[2*i+1]!=oracle[i])return 25;
  v=next;next=(spx_vec_v1){0};spx_sort_active=1;
  if(spx_copy_vec_sort_keys(&c,&v,2,tags,1,key,desc,&next)!=SPX_STATUS_SUCCESS)return 26;
  spx_sort_active=0;words=(uint64_t*)(void*)next.ptr;
  for(unsigned i=0;i<8;++i)if(words[2*i]!=7-i||words[2*i+1]!=oracle[7-i])return 27;
  spx_vec_drop(&c,&next);
 }
}
