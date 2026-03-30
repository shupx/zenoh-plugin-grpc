#include <iostream>

#include "common.hpp"

int main(int argc, char** argv) {
    auto session = zenoh_grpc::Session::connect(example_endpoint(argc, argv));

    zenoh_grpc::Session::PublisherOptions pub_options;
    pub_options.encoding = "text/plain";
    pub_options.priority = zenoh_grpc::Priority::Data;

    auto publisher = session.declare_publisher("demo/example/cpp", pub_options);
    std::cout << "publisher ready on " << example_endpoint(argc, argv) << std::endl;

    for (int i = 0; i < 500000; ++i) {
        std::string message = "hello from cpp " + std::to_string(i);
        zenoh_grpc::Publisher::PutOptions put_options;
        put_options.encoding = "text/plain";
        publisher.put(std::vector<std::uint8_t>(message.begin(), message.end()), put_options);
        std::cout << "published: " << message
                  << " dropped=" << publisher.send_dropped_count() << std::endl;
        example_sleep_ms(500);
    }

    return 0;
}
