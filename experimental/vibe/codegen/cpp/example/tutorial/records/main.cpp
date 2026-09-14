/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include QUENT_CPP_BRIDGE_HEADER

int run_example() {
  auto context = quent::Context::none();
  auto task = context.task_observer()->create();
  auto batch = context.batch_observer()->create();

  task.started();
  task.ended(quent::task::Ended{
      .result = quent::records::WorkResult{
          .success = true,
          .items_processed = 128,
      },
  });

  batch.started();
  batch.ended(quent::batch::Ended{
      .result = quent::records::WorkResult{
          .success = true,
          .items_processed = 512,
      },
  });
  return 0;
}

#ifndef QUENT_TUTORIAL_LIBRARY
int main() { return run_example(); }
#endif
