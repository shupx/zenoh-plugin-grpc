#include <iostream>

#include <zenoh_grpc/session.hpp>

int main() {
    auto session = zenoh_grpc::Session::open_tcp("127.0.0.1:7335");
    if (!session.valid()) {
        return 1;
    }

    auto subscriber = session.declare_subscriber("demo/example/**");
    auto sample = subscriber.recv();
    std::cout << sample.key_expr << " => "
              << std::string(sample.payload.begin(), sample.payload.end()) << std::endl;
    return 0;
}
