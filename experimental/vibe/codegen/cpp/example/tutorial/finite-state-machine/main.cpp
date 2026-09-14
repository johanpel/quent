/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include QUENT_CPP_BRIDGE_HEADER

int run_example() {
  auto context = quent::Context::none();
  auto queued = std::move(context.job_observer()->create()).queued();
  auto loading = std::move(queued).loading_input();
  auto running = std::move(loading).running();
  auto completed = std::move(running).completed();
  return 0;
}

#ifndef QUENT_TUTORIAL_LIBRARY
int main() { return run_example(); }
#endif
