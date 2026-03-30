#include <stdio.h>

#include "common.h"

int main(int argc, char** argv) {
    const char* endpoint = zgrpc_example_endpoint(argc, argv);
    zgrpc_session_t* session = zgrpc_session_connect(endpoint);
    if (session == NULL) {
        fprintf(stderr, "failed to connect to %s\n", endpoint);
        return 1;
    }

    zgrpc_queryable_options_t opts;
    zgrpc_queryable_options_default(&opts);
    opts.complete = false;
    opts.allowed_origin = ZGRPC_LOCALITY_ANY;

    zgrpc_queryable_t* queryable =
        zgrpc_declare_queryable_with_options(session, "demo/query/**", &opts);
    if (queryable == NULL) {
        fprintf(stderr, "failed to declare queryable\n");
        zgrpc_close(session);
        return 1;
    }

    printf("queryable ready on %s\n", endpoint);
    while (1) {
        zgrpc_query_ex_t query = {0};
        if (zgrpc_queryable_recv_ex(queryable, &query) != 0) {
            fprintf(stderr, "query receive failed\n");
            break;
        }

        zgrpc_example_print_query(&query);

        zgrpc_query_reply_options_t reply_opts;
        zgrpc_query_reply_options_default(&reply_opts);
        reply_opts.encoding = "text/plain";

        const char* reply1 = "this is a reply1 from c queryable";
        zgrpc_queryable_reply_with_options(queryable, query.query_id, query.key_expr,
                                           (const uint8_t*)reply1, strlen(reply1), &reply_opts);
        printf("reply 1 sent for query %" PRIu64 "\n", query.query_id);

        zgrpc_example_sleep_ms(1000);

        const char* reply2 = "this is a reply2 from c queryable";
        zgrpc_queryable_reply_with_options(queryable, query.query_id, query.key_expr,
                                           (const uint8_t*)reply2, strlen(reply2), &reply_opts);
        printf("reply 2 sent for query %" PRIu64 "\n", query.query_id);

        zgrpc_queryable_finish(queryable, query.query_id);
        zgrpc_query_ex_free(&query);
    }

    zgrpc_queryable_undeclare(queryable);
    zgrpc_close(session);
    return 0;
}
