#include <iostream>

#include "common.hpp"

int main(int argc, char** argv) {
    auto session = zenoh_grpc::Session::connect(example_endpoint(argc, argv));

    zenoh_grpc::Session::SubscriberOptions options;
    options.allowed_origin = zenoh_grpc::Locality::Any;
    auto subscriber = session.declare_subscriber("demo/example/**", options);

    std::cout << "subscriber ready on " << example_endpoint(argc, argv) << std::endl;
    for (int i = 0; i < 500; ++i) {
        auto sample = subscriber.recv();
        print_sample(sample);
        std::cout << "dropped=" << subscriber.dropped_count() << std::endl;
    }

    return 0;
}
