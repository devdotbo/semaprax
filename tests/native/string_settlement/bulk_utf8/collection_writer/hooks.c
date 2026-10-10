/* Only generated code is wrapped. Preparation, oracle construction and probe
 * reporting use unwrapped libc. No counter alters an ordinary runtime cap. */
static bool writer_phase;
static size_t writer_buffer_count,writer_buffer_size,writer_string_count,writer_string_size;
#undef calloc
static void *writer_calloc(size_t count,size_t size) {
    void *result=fixture_calloc(count,size);
    if(writer_phase){
        ++writer_buffer_count;writer_buffer_size=count*size;
        /* Observe actual explicit copies originating in this exact buffer. */
        fixture_input_start=result;fixture_input_length=count*size;
    }
    return result;
}
#undef malloc
static void *writer_malloc(size_t size) {
    if(writer_phase){++writer_string_count;writer_string_size=size;}
    return fixture_bulk_malloc(size);
}
#define calloc writer_calloc
#define malloc writer_malloc
static void begin_phase(void) {
    writer_buffer_count=writer_buffer_size=writer_string_count=writer_string_size=0;
    fixture_watch(NULL,0);writer_phase=true;
}
