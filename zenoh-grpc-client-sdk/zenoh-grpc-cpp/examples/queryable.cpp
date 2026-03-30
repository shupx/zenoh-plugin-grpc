#include <iostream>

#include "common.hpp"

int main(int argc, char** argv) {
    auto session = zenoh_grpc::Session::connect(example_endpoint(argc, argv));

    zenoh_grpc::Session::QueryableOptions options;
    options.complete = false;
    options.allowed_origin = zenoh_grpc::Locality::Any;
    auto queryable = session.declare_queryable("demo/query/**", options);

    std::cout << "queryable ready on " << example_endpoint(argc, argv) << std::endl;
    while (true) {
        auto query = queryable.recv();
        std::cout << "query id=" << query.query_id() << " selector=" << query.selector()
                  << " key=" << query.key_expr() << " parameters=" << query.parameters()
                  << " payload=" << bytes_to_string(query.payload())
                  << " encoding=" << query.encoding() << std::endl;

        zenoh_grpc::Query::ReplyOptions reply_options;
        reply_options.encoding = "text/plain";

        const std::string reply1 = "this is a reply1 from c++ queryable";
        query.reply(query.key_expr(),
                    std::vector<std::uint8_t>(reply1.begin(), reply1.end()),
                    reply_options);
        std::cout << "reply 1 sent" << std::endl;

        example_sleep_ms(1000);

        const std::string reply2 = "this is a reply2 from c++ queryable";
        query.reply(query.key_expr(),
                    std::vector<std::uint8_t>(reply2.begin(), reply2.end()),
                    reply_options);
        std::cout << "reply 2 sent" << std::endl;
        
        // The query will be automatically finished when the Query object is destructed, but we can also finish it explicitly here.
        query.finish();
    }

    return 0;
}
