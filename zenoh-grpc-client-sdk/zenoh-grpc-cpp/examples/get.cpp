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
    for (const auto& reply : replies) {
        print_reply(reply);
    }

    return 0;
}
