/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include "quent-tutorial-log-sink-cpp-bridge/gen/quent.hpp"

int main() {
  auto context = quent::Context::none();
  auto log = context.app_log_observer()->handle();

  // A logging API would typically use a macro to capture __FILE__, __LINE__,
  // and any other call-site context required by the schema.
  log.info(quent::app_log::Info{
      .message = "application started",
      .target = "startup",
      .file = __FILE__,
      .line = __LINE__,
      .module = "main",
      .thread_name = "main",
  });
  log.warning(quent::app_log::Warning{
      .message = "retrying request",
      .target = "network",
      .file = __FILE__,
      .line = __LINE__,
      .module = "main",
      .thread_name = "main",
      .category = "transient",
  });
  return 0;
}
