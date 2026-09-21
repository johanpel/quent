/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-fsm-dynamic-state-cpp-bridge/gen/quent.hpp"

quent::DynamicFsmHandle<quent::Job> prepare_job(
    quent::FsmHandle<quent::Job, quent::job_state::Queued> job,
    bool restore_from_checkpoint) {
  if (restore_from_checkpoint) {
    return std::move(job).restoring_checkpoint().into_dynamic();
  }
  return std::move(job).loading_input().into_dynamic();
}

int main(int argc, char**) {
  const bool restore_from_checkpoint = argc > 1;
  auto context = quent::Context::none();
  auto queued = context.job_observer()->handle().queued();
  auto job = prepare_job(std::move(queued), restore_from_checkpoint);
  job.running();
  job.completed();
  return 0;
}
