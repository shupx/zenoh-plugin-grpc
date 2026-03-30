#pragma once

#include <chrono>
#include <cstdint>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

#include <zenoh_grpc/session.hpp>

inline std::string example_endpoint(int argc, char** argv) {
    return argc > 1 ? std::string(argv[1]) : std::string("unix:///tmp/zenoh-grpc.sock");
}

inline void example_sleep_ms(int ms) { std::this_thread::sleep_for(std::chrono::milliseconds(ms)); }

inline std::string bytes_to_string(const std::vector<std::uint8_t>& value) {
    return std::string(value.begin(), value.end());
}

inline void print_sample(const zenoh_grpc::Sample& sample) {
    std::cout << "key=" << sample.key_expr << " payload=" << bytes_to_string(sample.payload)
              << " encoding=" << sample.encoding;
    if (!sample.attachment.empty()) {
        std::cout << " attachment=" << bytes_to_string(sample.attachment);
    }
    if (!sample.timestamp.empty()) {
        std::cout << " timestamp=" << sample.timestamp;
    }
    if (sample.source_info.has_value()) {
        std::cout << " source=" << sample.source_info->id << "/" << sample.source_info->sequence;
    }
    std::cout << std::endl;
}

inline void print_reply(const zenoh_grpc::Reply& reply) {
    if (reply.sample.has_value()) {
        std::cout << "sample: ";
        print_sample(*reply.sample);
    } else if (reply.error.has_value()) {
        std::cout << "error: " << bytes_to_string(reply.error->payload)
                  << " encoding=" << reply.error->encoding << std::endl;
    }
}
