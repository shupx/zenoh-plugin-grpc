#include "zenoh_grpc.h"

#include <stdio.h>

int main() {
    zgrpc_session_t* session = zgrpc_open_tcp("127.0.0.1:7335");
    if (session == NULL) {
        return 1;
    }

    zgrpc_subscriber_t* sub = zgrpc_declare_subscriber(session, "demo/example/**");
    if (sub == NULL) {
        zgrpc_close(session);
        return 1;
    }

    zgrpc_sample_t sample = {0};
    if (zgrpc_subscriber_recv(sub, &sample) != 0) {
        zgrpc_subscriber_undeclare(sub);
        zgrpc_close(session);
        return 1;
    }

    printf("%s => %.*s\n", sample.key_expr, (int)sample.payload_len, (const char*)sample.payload);
    zgrpc_sample_free(&sample);
    zgrpc_subscriber_undeclare(sub);
    zgrpc_close(session);
    return 0;
}
