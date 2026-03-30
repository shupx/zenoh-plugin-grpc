cmake_minimum_required(VERSION 3.16)
project(zenohgrpcxx LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(ZENOH_GRPCXX_BUILD_EXAMPLES "Build zenoh-grpc-cpp examples" ON)
set(ZENOH_GRPC_C_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../zenoh-grpc-c/include" CACHE PATH "Directory containing zenoh_grpc.h")
set(ZENOH_GRPC_C_LIBDIR "${CMAKE_CURRENT_SOURCE_DIR}/../../target/release" CACHE PATH "Directory containing libzenohgrpc")

add_library(zenohgrpcxx INTERFACE)
target_include_directories(
    zenohgrpcxx
    INTERFACE
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<BUILD_INTERFACE:${ZENOH_GRPC_C_INCLUDE_DIR}>
        $<INSTALL_INTERFACE:include>
)

if(ZENOH_GRPCXX_BUILD_EXAMPLES)
    find_library(ZENOH_GRPC_C_LIBRARY NAMES zenohgrpc PATHS "${ZENOH_GRPC_C_LIBDIR}" NO_DEFAULT_PATH)
    if(NOT ZENOH_GRPC_C_LIBRARY)
        message(FATAL_ERROR "Could not find libzenohgrpc in ${ZENOH_GRPC_C_LIBDIR}. Build it first with cargo build -p zenoh-grpc-c --release.")
    endif()

    add_library(zenohgrpc_c SHARED IMPORTED GLOBAL)
    set_target_properties(zenohgrpc_c PROPERTIES IMPORTED_LOCATION "${ZENOH_GRPC_C_LIBRARY}")
    target_include_directories(zenohgrpc_c INTERFACE "${ZENOH_GRPC_C_INCLUDE_DIR}")

    set(ZENOH_GRPCXX_EXAMPLES
        pub
        sub
        sub_callback
        queryable
        queryable_callback
        querier
        get
    )

    foreach(example ${ZENOH_GRPCXX_EXAMPLES})
        add_executable(${example} "examples/${example}.cpp")
        target_include_directories(${example} PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/examples")
        target_link_libraries(${example} PRIVATE zenohgrpcxx zenohgrpc_c)
    endforeach()
endif()

install(TARGETS zenohgrpcxx EXPORT zenohgrpcxxTargets)
install(DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}/include/" DESTINATION include)
install(FILES "${ZENOH_GRPC_C_INCLUDE_DIR}/zenoh_grpc.h" DESTINATION include)
install(EXPORT zenohgrpcxxTargets DESTINATION lib/cmake/zenohgrpcxx NAMESPACE zenoh_grpc::)
