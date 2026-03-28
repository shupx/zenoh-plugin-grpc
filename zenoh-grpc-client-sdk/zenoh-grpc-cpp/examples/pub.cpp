#include <cstdint>
#include <vector>

#include <zenoh_grpc/session.hpp>

int main() {
    auto session = zenoh_grpc::Session::open_tcp("127.0.0.1:7335");
    if (!session.valid()) {
        return 1;
    }

    auto publisher = session.declare_publisher("demo/example/cpp");
    publisher.put(std::vector<std::uint8_t>{'h', 'e', 'l', 'l', 'o'});
    return 0;
}
