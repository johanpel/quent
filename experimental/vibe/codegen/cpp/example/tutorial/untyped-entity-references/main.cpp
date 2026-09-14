/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include QUENT_CPP_BRIDGE_HEADER

int run_example() {
  auto context = quent::Context::none();
  auto worker = context.worker_observer()->create();
  worker.started();

  auto task = context.task_observer()->create();
  task.started(quent::task::Started{.source = worker.id().raw()});
  task.ended();
  worker.ended();
  return 0;
}

#ifndef QUENT_TUTORIAL_LIBRARY
int main() { return run_example(); }
#endif
