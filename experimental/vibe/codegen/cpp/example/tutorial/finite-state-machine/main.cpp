/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-finite-state-machine-cpp-bridge/gen/quent.hpp"

int main() {
  auto context = quent::Context::none();
  auto queued = std::move(context.job_observer()->create()).queued();
  auto loading = std::move(queued).loading_input();
  auto running = std::move(loading).running();
  auto completed = std::move(running).completed();
  return 0;
}
