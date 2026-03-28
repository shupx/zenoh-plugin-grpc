#include "zenoh_grpc.h"

#include <stdint.h>

int main() {
    zgrpc_session_t* session = zgrpc_open_tcp("127.0.0.1:7335");
    if (session == NULL) {
        return 1;
    }

    const uint8_t payload[] = "hello from c";
    int rc = zgrpc_session_put(session, "demo/example/c", payload, sizeof(payload) - 1);
    zgrpc_close(session);
    return rc == 0 ? 0 : 1;
}
