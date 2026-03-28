cmake_minimum_required(VERSION 3.16)
project(zenohgrpcxx LANGUAGES C CXX)
set(CMAKE_CXX_STANDARD 17)

add_library(zenohgrpcxx INTERFACE)
target_include_directories(zenohgrpcxx INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/include")
