/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-job-workload-cpp-bridge/gen/quent.hpp"

int main() {
  auto context = quent::Context::none();
  auto worker = context.worker_observer()->create();
  worker.ready(quent::worker::Ready{
      .name = "worker-1",
      .limits = quent::records::WorkerBounds{.threads = 16},
  });

  auto queued = std::move(context.job_observer()->create())
                    .queued(quent::job::Queued{
                        .name = "compile",
                        .requested_threads = 4,
                    });
  auto running = std::move(queued).running(quent::job::Running{
      .worker = quent::refs::WorkerUsageRef{
          .target = worker.id(),
          .data = quent::records::WorkerUsage{.threads = 4},
      },
  });
  auto completed = std::move(running).completed();
  return 0;
}
