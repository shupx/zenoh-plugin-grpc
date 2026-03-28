#include <cstdint>
#include <vector>

#include <zenoh_grpc/session.hpp>

int main() {
    auto session = zenoh_grpc::Session::open_tcp("127.0.0.1:7335");
    if (!session.valid()) {
        return 1;
    }

    auto queryable = session.declare_queryable("demo/query/**");
    auto query = queryable.recv();
    queryable.reply(query.query_id, "demo/query/value", std::vector<std::uint8_t>{'o', 'k'});
    return 0;
}
