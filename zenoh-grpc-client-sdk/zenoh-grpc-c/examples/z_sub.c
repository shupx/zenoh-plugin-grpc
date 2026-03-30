#include <stdio.h>

#include "common.h"

int main(int argc, char** argv) {
    const char* endpoint = zgrpc_example_endpoint(argc, argv);
    zgrpc_session_t* session = zgrpc_session_connect(endpoint);
    if (session == NULL) {
        fprintf(stderr, "failed to connect to %s\n", endpoint);
        return 1;
    }

    zgrpc_subscriber_options_t sub_opts;
    zgrpc_subscriber_options_default(&sub_opts);
    sub_opts.allowed_origin = ZGRPC_LOCALITY_ANY;

    zgrpc_subscriber_t* subscriber =
        zgrpc_declare_subscriber_with_options(session, "demo/example/**", &sub_opts);
    if (subscriber == NULL) {
        fprintf(stderr, "failed to declare subscriber\n");
        zgrpc_close(session);
        return 1;
    }

    printf("subscriber ready on %s\n", endpoint);
    for (int i = 0; i < 5; ++i) {
        zgrpc_subscriber_event_t event = {0};
        if (zgrpc_subscriber_recv_event(subscriber, &event) != 0) {
            fprintf(stderr, "receive failed\n");
            zgrpc_subscriber_undeclare(subscriber);
            zgrpc_close(session);
            return 1;
        }

        if (event.has_sample) {
            zgrpc_example_print_sample(&event.sample);
        }
        printf("dropped=%" PRIu64 "\n", zgrpc_subscriber_dropped_count(subscriber));
        zgrpc_subscriber_event_free(&event);
    }

    zgrpc_subscriber_undeclare(subscriber);
    zgrpc_close(session);
    return 0;
}
