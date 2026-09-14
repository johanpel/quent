/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-finite-state-machine-cpp-bridge/gen/quent.hpp"

int main() {
  auto context = quent::Context::none();

  quent::job::JobHandle job = context.job_observer()->create();

  // A transition consumes its move-only state handle and returns the handle
  // type for the new state. Named C++ objects require std::move to be consumed.
  quent::job::JobQueuedHandle queued = std::move(job).queued();
  quent::job::JobLoadingInputHandle loading =
      std::move(queued).loading_input();
  quent::job::JobRunningHandle running = std::move(loading).running();
  quent::job::JobCompletedHandle completed = std::move(running).completed();
  return 0;
}
