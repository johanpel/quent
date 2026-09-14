/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-unit-resource-cpp-bridge/gen/quent.hpp"

int run_example() {
  auto context = quent::Context::none();
  auto pool = context.thread_pool_observer()->create();
  pool.created();

  auto thread = context.thread_observer()->create();
  thread.registered(quent::thread::Registered{.pool = pool.id()});

  auto running = std::move(context.task_observer()->create())
                     .running(quent::task::Running{
                         .thread = quent::refs::ThreadUsageRef{
                             .target = thread.id(),
                             .data = quent::records::ThreadUsage{},
                         },
                     });
  auto completed = std::move(running).completed();
  return 0;
}

#ifndef QUENT_TUTORIAL_LIBRARY
int main() { return run_example(); }
#endif
