/* Test-owned physical owner census, outside measured/selected allocator policy. */
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void *physical_owners[128];
static size_t physical_live;
static void *probe_malloc(size_t bytes) {
    void *p=malloc(bytes);if(p==NULL)return NULL;
    for(unsigned i=0;i<128;++i)if(physical_owners[i]==NULL){physical_owners[i]=p;++physical_live;return p;}
    abort();
}
static void probe_free(void *p) {
    if(p==NULL)return;
    for(unsigned i=0;i<128;++i)if(physical_owners[i]==p){physical_owners[i]=NULL;assert(physical_live!=0);--physical_live;free(p);return;}
    abort();
}
#define malloc probe_malloc
#define free probe_free
