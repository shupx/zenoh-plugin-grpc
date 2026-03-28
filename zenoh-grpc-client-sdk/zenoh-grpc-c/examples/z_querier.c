#include "zenoh_grpc.h"

#include <stdio.h>

int main() {
    zgrpc_session_t* session = zgrpc_open_tcp("127.0.0.1:7335");
    if (session == NULL) {
        return 1;
    }

    zgrpc_querier_t* querier = zgrpc_declare_querier(session, "demo/query/**");
    if (querier == NULL) {
        zgrpc_close(session);
        return 1;
    }

    zgrpc_reply_t reply = {0};
    if (zgrpc_querier_get(querier, "", &reply) != 0) {
        zgrpc_querier_undeclare(querier);
        zgrpc_close(session);
        return 1;
    }

    printf("reply: %.*s\n", (int)reply.payload_len, (const char*)reply.payload);
    zgrpc_reply_free(&reply);
    zgrpc_querier_undeclare(querier);
    zgrpc_close(session);
    return 0;
}
