/* Payload stores are ordinary bytes_set operations, not claimed memcpy work.
 * Report carrier copies remain separately observed; final buffer->String memcpy
 * is selected by the allocator's actual source interval, never a guessed count.
 * String physical OOM remains fatal and is not tested as returned recovery here. */
static void emit_metrics(size_t rows,const char *phase,size_t allocations,size_t frees) {
    (void)printf("{\"rows\":%zu,\"phase\":\"%s\",\"allocations\":%zu,\"frees\":%zu,"
      "\"bytes_reservations\":%zu,\"reserved_payload_bytes\":%zu,\"string_allocations\":%zu,"
      "\"string_physical_bytes\":%zu,\"explicit_memcpy_calls\":%zu,\"explicit_memcpy_bytes\":%zu,"
      "\"final_buffer_copy_calls\":%zu,\"final_buffer_copy_bytes\":%zu,\"payload_store_count\":null}\n",
      rows,phase,fixture_allocations-allocations,fixture_frees-frees,
      writer_buffer_count,writer_buffer_size,writer_string_count,writer_string_size,
      fixture_copy_calls,fixture_copy_bytes,fixture_input_copy_calls,fixture_input_copy_bytes);
}
static void case_run(struct spx_context *context,size_t rows,const uint8_t *expected,size_t length) {
    struct REPORT input={0};
    REQUIRE(PREPARE(context,rows,&input)==SPX_STATUS_SUCCESS);
    size_t baseline_live=fixture_live;
    spx_slice_u8_v1 oracle={.ptr=expected,.len=length};
    /* Exact length is independently known from immutable host oracle bytes. */
    uint64_t required=0;
    size_t allocated=fixture_allocations,freed=fixture_frees;
    begin_phase();
    REQUIRE(REQUIRED(context,&input,&required)==SPX_STATUS_SUCCESS && required==length);
    writer_phase=false;
    REQUIRE(fixture_allocations==allocated && fixture_frees==freed && fixture_live==baseline_live);
    REQUIRE(writer_buffer_count==0 && writer_string_count==0 && fixture_input_copy_bytes==0);
    emit_metrics(rows,"preflight",allocated,freed);
    allocated=fixture_allocations;freed=fixture_frees;
    int64_t result=-1;
    begin_phase();
    REQUIRE(RUN(context,&input,required-1,oracle,&result)==SPX_STATUS_SUCCESS && result==1);
    writer_phase=false;
    REQUIRE(fixture_allocations==allocated && fixture_frees==freed && fixture_live==baseline_live);
    REQUIRE(writer_buffer_count==0 && writer_string_count==0 && fixture_input_copy_bytes==0);
    emit_metrics(rows,"one-short-refusal",allocated,freed);
    allocated=fixture_allocations;freed=fixture_frees;result=-1;
    begin_phase();
    REQUIRE(RUN(context,&input,required,oracle,&result)==SPX_STATUS_SUCCESS && result==2);
    writer_phase=false;
    REQUIRE(fixture_allocations-allocated==2 && fixture_frees-freed==2 && fixture_live==baseline_live);
    REQUIRE(writer_buffer_count==1 && writer_buffer_size==length);
    REQUIRE(writer_string_count==1 && writer_string_size==offsetof(struct spx_string_v10,data)+length+1);
    REQUIRE(fixture_input_copy_calls==1 && fixture_input_copy_bytes==length);
    REQUIRE(context->call_depth==0 && context->borrowed_str_depth==0);
    emit_metrics(rows,"exact-generated-encode-and-byte-oracle",allocated,freed);
    /* Second authentic render catches persistent input corruption, outside the
     * reported measurement. No materialization counter or context is reset. */
    REQUIRE(RUN(context,&input,required,oracle,&result)==SPX_STATUS_SUCCESS && result==2);
    REQUIRE(fixture_live==baseline_live);
    REQUIRE(CONSUME(context,&input,&result)==SPX_STATUS_SUCCESS && result==0);
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
    REQUIRE(context->call_depth==0 && context->borrowed_str_depth==0);
}
