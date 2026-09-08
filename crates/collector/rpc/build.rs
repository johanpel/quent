// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

fn main() {
    let collect_events = tonic_build::manual::Method::builder()
        .name("collect_events")
        .route_name("CollectEvents")
        .input_type("crate::EventBatch")
        .output_type("crate::CollectResponse")
        .codec_path("crate::CollectorCodec")
        .client_streaming()
        .build();
    let collector = tonic_build::manual::Service::builder()
        .name("Collector")
        .package("quent.collector.v1alpha")
        .method(collect_events)
        .build();

    tonic_build::manual::Builder::new().compile(&[collector]);
}
