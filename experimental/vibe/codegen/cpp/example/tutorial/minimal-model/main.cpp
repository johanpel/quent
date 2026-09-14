/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-minimal-model-cpp-bridge/gen/quent.hpp"

int run_example() {
  auto context = quent::Context::none();
  auto task = context.task_observer()->create();
  task.started();
  task.ended();
  return 0;
}

#ifndef QUENT_TUTORIAL_LIBRARY
int main() { return run_example(); }
#endif
