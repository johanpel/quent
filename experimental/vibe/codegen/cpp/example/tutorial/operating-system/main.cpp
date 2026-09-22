/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-operating-system-cpp-bridge/gen/quent.hpp"

#include <cerrno>
#include <cstdint>
#include <system_error>
#include <thread>
#include <utility>

#if defined(_WIN32)
#include <windows.h>
#elif defined(__APPLE__)
#include <pthread.h>
#include <unistd.h>
#elif defined(__linux__)
#include <sys/syscall.h>
#include <unistd.h>
#else
#error "This example requires Linux, macOS, or Windows"
#endif

namespace {

std::uint32_t current_process_id() {
#if defined(_WIN32)
  return static_cast<std::uint32_t>(GetCurrentProcessId());
#else
  return static_cast<std::uint32_t>(getpid());
#endif
}

std::uint64_t current_thread_id() {
#if defined(_WIN32)
  return static_cast<std::uint64_t>(GetCurrentThreadId());
#elif defined(__APPLE__)
  std::uint64_t native_id = 0;
  const int error = pthread_threadid_np(nullptr, &native_id);
  if (error != 0) {
    throw std::system_error(error, std::generic_category(),
                            "pthread_threadid_np");
  }
  return native_id;
#else
  const long native_id = syscall(SYS_gettid);
  if (native_id == -1) {
    throw std::system_error(errno, std::generic_category(), "gettid");
  }
  return static_cast<std::uint64_t>(native_id);
#endif
}

}  // namespace

int main() {
  auto context = quent::Context::none();

  auto process = context.process_observer()->handle();
  process.started(quent::process::Started{
      .process = quent::records::QuentOsProcess{
          .native_id = current_process_id(),
      },
  });

  auto thread = context.thread_observer()->handle();
  std::thread worker([thread = std::move(thread),
                      process = process.id()]() mutable {
    thread.started(quent::thread::Started{
        .thread = quent::records::QuentOsThread{
            .native_id = current_thread_id(),
        },
        .process = process,
    });
  });
  worker.join();
  return 0;
}
