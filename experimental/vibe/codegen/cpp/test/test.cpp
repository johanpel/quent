/*
 * SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

#include QUENT_CPP_BRIDGE_HEADER

quent::DynamicAttributes make_dynamic_attributes();
int run_example();

extern "C" int quent_demo_cpp_smoke() { return run_example(); }

extern "C" int quent_demo_cpp_dynamic_values(const char *output_dir) {
  auto context = quent::Context::ndjson(output_dir);
  auto worker = context.worker_observer()->create();
  worker.declaration(quent::worker::Declaration{
      .instance_name = "dynamic_values",
      .cluster = quent::cluster::ClusterId(context.id()),
      .details = quent::records::Details{
          .version = "1.0",
          .custom = make_dynamic_attributes(),
      },
  });
  return 0;
}
