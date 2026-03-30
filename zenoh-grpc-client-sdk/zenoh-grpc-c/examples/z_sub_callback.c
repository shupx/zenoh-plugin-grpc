#include <stdio.h>

#include "common.h"

static void on_sample(const zgrpc_subscriber_event_t* event, void* context) {
    (void)context;
    printf("callback: ");
    if (event->has_sample) {
        zgrpc_example_print_sample(&event->sample);
    } else {
        printf("<empty event>\n");
    }
}

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

    zgrpc_subscriber_t* subscriber = zgrpc_declare_subscriber_with_callback(
        session, "demo/example/**", on_sample, NULL, NULL, &sub_opts);
    if (subscriber == NULL) {
        fprintf(stderr, "failed to declare callback subscriber\n");
        zgrpc_close(session);
        return 1;
    }

    printf("callback subscriber ready on %s\n", endpoint);
    zgrpc_example_sleep_ms(5000);

    zgrpc_subscriber_undeclare(subscriber);
    zgrpc_close(session);
    return 0;
}
