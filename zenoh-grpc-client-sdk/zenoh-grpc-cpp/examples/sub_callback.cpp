#include <iostream>

#include "common.hpp"

static void on_sample(const zenoh_grpc::SubscriberEvent& event) {
    std::cout << "callback: ";
    if (event.sample.has_value()) {
        print_sample(*event.sample);
    } else {
        std::cout << "<empty event>" << std::endl;
    }
}

int main(int argc, char** argv) {
    auto session = zenoh_grpc::Session::connect(example_endpoint(argc, argv));

    zenoh_grpc::Session::SubscriberOptions options;
    options.allowed_origin = zenoh_grpc::Locality::Any;
    auto subscriber = session.declare_subscriber("demo/example/**", on_sample, options);

    std::cout << "callback subscriber ready on " << example_endpoint(argc, argv) << std::endl;
    example_sleep_ms(5000000);
    (void)subscriber;
    return 0;
}
