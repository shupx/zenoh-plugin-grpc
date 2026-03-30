#pragma once

#include <inttypes.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include "zenoh_grpc.h"

static inline const char* zgrpc_example_endpoint(int argc, char** argv) {
    return argc > 1 ? argv[1] : "unix:///tmp/zenoh-grpc.sock";
}

static inline void zgrpc_example_sleep_ms(unsigned int ms) { usleep(ms * 1000); }

static inline void zgrpc_example_print_bytes(const uint8_t* value, size_t len) {
    if (value == NULL || len == 0) {
        printf("<empty>");
        return;
    }
    printf("%.*s", (int)len, (const char*)value);
}

static inline void zgrpc_example_print_sample(const zgrpc_sample_ex_t* sample) {
    printf("key=%s payload=", sample->key_expr != NULL ? sample->key_expr : "<null>");
    zgrpc_example_print_bytes(sample->payload, sample->payload_len);
    printf(" encoding=%s", sample->encoding != NULL ? sample->encoding : "<empty>");
    if (sample->attachment_len > 0) {
        printf(" attachment=");
        zgrpc_example_print_bytes(sample->attachment, sample->attachment_len);
    }
    if (sample->timestamp != NULL) {
        printf(" timestamp=%s", sample->timestamp);
    }
    if (sample->source_info.is_valid) {
        printf(" source=%s/%" PRIu64,
               sample->source_info.id != NULL ? sample->source_info.id : "<null>",
               sample->source_info.sequence);
    }
    printf("\n");
}

static inline void zgrpc_example_print_query(const zgrpc_query_ex_t* query) {
    printf("query id=%" PRIu64 " selector=%s key=%s parameters=%s payload=",
           query->query_id,
           query->selector != NULL ? query->selector : "<null>",
           query->key_expr != NULL ? query->key_expr : "<null>",
           query->parameters != NULL ? query->parameters : "<empty>");
    zgrpc_example_print_bytes(query->payload, query->payload_len);
    printf(" encoding=%s", query->encoding != NULL ? query->encoding : "<empty>");
    if (query->attachment_len > 0) {
        printf(" attachment=");
        zgrpc_example_print_bytes(query->attachment, query->attachment_len);
    }
    printf("\n");
}

static inline void zgrpc_example_print_reply(const zgrpc_reply_ex_t* reply) {
    if (reply->has_sample) {
        printf("sample: ");
        zgrpc_example_print_sample(&reply->sample);
    } else if (reply->has_error) {
        printf("error: ");
        zgrpc_example_print_bytes(reply->error.payload, reply->error.payload_len);
        printf(" encoding=%s\n", reply->error.encoding != NULL ? reply->error.encoding : "<empty>");
    } else {
        printf("<empty reply>\n");
    }
}
