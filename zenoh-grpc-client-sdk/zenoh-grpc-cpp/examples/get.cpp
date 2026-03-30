#include <iostream>

#include "common.hpp"

int main(int argc, char** argv) {
    auto session = zenoh_grpc::Session::connect(example_endpoint(argc, argv));

    zenoh_grpc::Session::GetOptions options;
    options.payload = std::vector<std::uint8_t>{'h', 'a', 'h', 'a', 'h', 'a'};
    options.consolidation = zenoh_grpc::ConsolidationMode::None;
    options.encoding = "text/plain";
    options.timeout_ms = 3000;

    auto replies = session.get("demo/query/c", options);
    while (true) {
        auto reply = replies.try_recv();
        if (reply.has_value()) {
            print_reply(*reply);
            continue;
        }
        if (replies.is_closed()) {
            break;
        }
        example_sleep_ms(100);
    }

    return 0;
}
