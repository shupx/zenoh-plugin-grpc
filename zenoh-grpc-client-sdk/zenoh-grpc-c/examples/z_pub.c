#include <inttypes.h>
#include <stdio.h>

#include "common.h"

int main(int argc, char** argv) {
    const char* endpoint = zgrpc_example_endpoint(argc, argv);
    zgrpc_session_t* session = zgrpc_session_connect(endpoint);
    if (session == NULL) {
        fprintf(stderr, "failed to connect to %s\n", endpoint);
        return 1;
    }

    zgrpc_publisher_options_t pub_opts;
    zgrpc_publisher_options_default(&pub_opts);
    pub_opts.encoding = "text/plain";
    pub_opts.priority = ZGRPC_PRIORITY_DATA;

    zgrpc_publisher_t* publisher =
        zgrpc_declare_publisher_with_options(session, "demo/example/c", &pub_opts);
    if (publisher == NULL) {
        fprintf(stderr, "failed to declare publisher\n");
        zgrpc_close(session);
        return 1;
    }

    printf("publisher ready on %s\n", endpoint);
    for (int i = 0; i < 5; ++i) {
        char message[128];
        snprintf(message, sizeof(message), "hello from c %d", i);

        zgrpc_publisher_put_options_t put_opts;
        zgrpc_publisher_put_options_default(&put_opts);
        put_opts.encoding = "text/plain";

        if (zgrpc_publisher_put_with_options(
                publisher, (const uint8_t*)message, strlen(message), &put_opts) != 0) {
            fprintf(stderr, "publish failed\n");
            zgrpc_publisher_undeclare(publisher);
            zgrpc_close(session);
            return 1;
        }

        printf("published: %s (dropped=%" PRIu64 ")\n", message,
               zgrpc_publisher_send_dropped_count(publisher));
        zgrpc_example_sleep_ms(500);
    }

    zgrpc_publisher_undeclare(publisher);
    zgrpc_close(session);
    return 0;
}
