# cmake-format: off
# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
# cmake-format: on

cmake_minimum_required(VERSION 3.24)
project(quent_instrumentation_latency_cpp LANGUAGES CXX)

if(NOT DEFINED QUENT_BRIDGE_LIBRARY OR NOT DEFINED QUENT_BRIDGE_INCLUDE)
  message(FATAL_ERROR "QUENT_BRIDGE_LIBRARY and QUENT_BRIDGE_INCLUDE are required")
endif()

find_package(benchmark CONFIG REQUIRED)
find_package(opentelemetry-cpp CONFIG REQUIRED COMPONENTS api sdk exporters_ostream)
find_package(quill CONFIG REQUIRED)
find_package(spdlog CONFIG REQUIRED)

add_library(quent_bench_cpp_bridge STATIC IMPORTED GLOBAL)
set_target_properties(quent_bench_cpp_bridge PROPERTIES
  IMPORTED_LOCATION "${QUENT_BRIDGE_LIBRARY}"
  INTERFACE_INCLUDE_DIRECTORIES "${QUENT_BRIDGE_INCLUDE}"
)

add_executable(quent_bench_cpp latency.cpp)
target_compile_features(quent_bench_cpp PRIVATE cxx_std_20)
target_link_libraries(quent_bench_cpp PRIVATE
  benchmark::benchmark
  opentelemetry-cpp::logs
  opentelemetry-cpp::ostream_log_record_exporter
  "$<LINK_LIBRARY:WHOLE_ARCHIVE,quent_bench_cpp_bridge>"
  quill::quill
  spdlog::spdlog
)

if(UNIX AND NOT APPLE)
  target_link_libraries(quent_bench_cpp PRIVATE dl m rt util)
endif()

get_filename_component(QUENT_BRIDGE_DIRECTORY "${QUENT_BRIDGE_LIBRARY}" DIRECTORY)
set_target_properties(quent_bench_cpp PROPERTIES
  BUILD_RPATH "${QUENT_BRIDGE_DIRECTORY}"
)
