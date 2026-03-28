#include <iostream>
#include <string>

#include <zenoh_grpc/session.hpp>

int main() {
    auto session = zenoh_grpc::Session::open_tcp("127.0.0.1:7335");
    if (!session.valid()) {
        return 1;
    }

    auto querier = session.declare_querier("demo/query/**");
    auto reply = querier.get();
    std::cout << std::string(reply.payload.begin(), reply.payload.end()) << std::endl;
    return 0;
}
