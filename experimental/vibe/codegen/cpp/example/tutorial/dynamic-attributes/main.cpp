/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-dynamic-attributes-cpp-bridge/gen/quent.hpp"

int run_example() {
  auto context = quent::Context::none();
  auto task = context.task_observer()->create();

  quent::DynamicAttributes started_details;
  started_details.add_string("queue", "priority");
  started_details.add_u64("attempt", 2);
  task.started(quent::task::Started{.details = std::move(started_details)});

  quent::DynamicAttributes ended_details;
  ended_details.add_bool("cached", false);
  ended_details.add_u64("items_processed", 128);
  task.ended(quent::task::Ended{.details = std::move(ended_details)});
  return 0;
}

#ifndef QUENT_TUTORIAL_LIBRARY
int main() { return run_example(); }
#endif
