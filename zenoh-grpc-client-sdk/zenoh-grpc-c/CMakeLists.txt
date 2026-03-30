cmake_minimum_required(VERSION 3.16)
project(zenohgrpc_c_examples LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(ZENOH_GRPC_C_LIBDIR "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release" CACHE PATH "Directory containing libzenohgrpc")
set(ZENOH_GRPC_C_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/include" CACHE PATH "Directory containing zenoh_grpc.h")
option(ZENOH_GRPC_C_BUILD_EXAMPLES "Build zenoh-grpc-c examples" ON)

find_library(ZENOH_GRPC_C_LIBRARY NAMES zenohgrpc PATHS "${ZENOH_GRPC_C_LIBDIR}" NO_DEFAULT_PATH)
if(NOT ZENOH_GRPC_C_LIBRARY)
    message(FATAL_ERROR "Could not find libzenohgrpc in ${ZENOH_GRPC_C_LIBDIR}. Build it first with cargo build -p zenoh-grpc-c --release.")
endif()

add_library(zenohgrpc_c SHARED IMPORTED GLOBAL)
set_target_properties(zenohgrpc_c PROPERTIES IMPORTED_LOCATION "${ZENOH_GRPC_C_LIBRARY}")
target_include_directories(zenohgrpc_c INTERFACE "${ZENOH_GRPC_C_INCLUDE_DIR}")

if(ZENOH_GRPC_C_BUILD_EXAMPLES)
    set(ZENOH_GRPC_C_EXAMPLES
        z_pub
        z_sub
        z_sub_callback
        z_queryable
        z_queryable_callback
        z_querier
        z_get
    )

    foreach(example ${ZENOH_GRPC_C_EXAMPLES})
        add_executable(${example} "examples/${example}.c")
        target_include_directories(${example} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/examples")
        target_link_libraries(${example} PRIVATE zenohgrpc_c)
    endforeach()
endif()
