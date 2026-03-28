#include "zenoh_grpc.h"

#include <stdint.h>

int main() {
    zgrpc_session_t* session = zgrpc_open_tcp("127.0.0.1:7335");
    if (session == NULL) {
        return 1;
    }

    zgrpc_queryable_t* queryable = zgrpc_declare_queryable(session, "demo/query/**");
    if (queryable == NULL) {
        zgrpc_close(session);
        return 1;
    }

    zgrpc_query_t query = {0};
    if (zgrpc_queryable_recv(queryable, &query) != 0) {
        zgrpc_queryable_undeclare(queryable);
        zgrpc_close(session);
        return 1;
    }

    const uint8_t payload[] = "reply from c";
    int rc = zgrpc_queryable_reply(queryable, query.query_id, "demo/query/value", payload, sizeof(payload) - 1);
    zgrpc_query_free(&query);
    zgrpc_queryable_undeclare(queryable);
    zgrpc_close(session);
    return rc == 0 ? 0 : 1;
}
