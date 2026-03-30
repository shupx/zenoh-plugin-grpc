#include <iostream>

#include "common.hpp"

int main(int argc, char** argv) {
    auto session = zenoh_grpc::Session::connect(example_endpoint(argc, argv));

    zenoh_grpc::Session::QuerierOptions options;
    options.consolidation = zenoh_grpc::ConsolidationMode::None;
    options.timeout_ms = 3000;
    auto querier = session.declare_querier("demo/query/**", options);

    zenoh_grpc::Querier::GetOptions get_options;
    get_options.payload = std::vector<std::uint8_t>{'h', 'a', 'h', 'a', 'h', 'a', 'h', 'a'};
    get_options.encoding = "text/plain";

    auto replies = querier.get_stream("", get_options);
    std::cout << "query sent, waiting for replies..." << std::endl;
    for (const auto& reply : replies) {
        print_reply(reply);
    }

    std::cout << "reply stream dropped=" << replies.dropped_count() << std::endl;
    return 0;
}
