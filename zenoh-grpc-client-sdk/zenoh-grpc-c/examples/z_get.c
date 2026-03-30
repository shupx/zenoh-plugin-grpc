#include <stdio.h>

#include "common.h"

int main(int argc, char** argv) {
    const char* endpoint = zgrpc_example_endpoint(argc, argv);
    zgrpc_session_t* session = zgrpc_session_connect(endpoint);
    if (session == NULL) {
        fprintf(stderr, "failed to connect to %s\n", endpoint);
        return 1;
    }

    zgrpc_session_get_options_t opts;
    zgrpc_session_get_options_default(&opts);
    opts.payload = (const uint8_t*)"hahaha";
    opts.payload_len = strlen("hahaha");
    opts.consolidation = ZGRPC_CONSOLIDATION_MODE_NONE;
    opts.encoding = "text/plain";
    opts.timeout_ms = 3000;

    zgrpc_reply_stream_t* replies = zgrpc_session_get(session, "demo/query/c", &opts);
    if (replies == NULL) {
        fprintf(stderr, "failed to open session get stream\n");
        zgrpc_close(session);
        return 1;
    }

    while (1) {
        zgrpc_reply_ex_t reply = {0};
        int rc = zgrpc_reply_stream_try_recv(replies, &reply);
        if (rc == 0) {
            zgrpc_example_print_reply(&reply);
            zgrpc_reply_ex_free(&reply);
        } else if (rc == 1) {
            if (zgrpc_reply_stream_is_closed(replies)) {
                break;
            }
            zgrpc_example_sleep_ms(100);
        } else {
            fprintf(stderr, "session get stream failed\n");
            break;
        }
    }

    zgrpc_reply_stream_drop(replies);
    zgrpc_close(session);
    return 0;
}
