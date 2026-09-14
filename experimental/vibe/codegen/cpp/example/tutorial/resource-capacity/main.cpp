/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include QUENT_CPP_BRIDGE_HEADER

int run_example() {
  auto context = quent::Context::none();
  auto memory = context.memory_observer()->create();
  memory.created();

  auto running = std::move(context.task_observer()->create())
                     .running(quent::task::Running{
                         .memory = quent::refs::MemoryUsageRef{
                             .target = memory.id(),
                             .data = quent::records::MemoryUsage{
                                 .bytes = 512'000'000,
                             },
                         },
                     });
  auto completed = std::move(running).completed();
  return 0;
}

#ifndef QUENT_TUTORIAL_LIBRARY
int main() { return run_example(); }
#endif
