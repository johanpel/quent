// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[allow(unused)]
mod demo {
    include!(concat!(env!("OUT_DIR"), "/demo.rs"));
}

use demo::{Connection, Context, Demo, Noop, Query, query_state};

#[test]
fn dynamic_to_typestate_retains_handle_on_mismatch() {
    let context = Context::<Demo>::try_new(Noop).unwrap();
    let connection = context.observer::<Connection>().handle();
    let dynamic = context
        .observer::<Query>()
        .handle()
        .submitted("select 1".to_owned(), connection.as_entity_ref())
        .into_dynamic();
    let query_id = dynamic.id();

    let mismatch = match dynamic.try_into::<query_state::Running>() {
        Ok(_) => panic!("submitted unexpectedly converted to running"),
        Err(error) => error,
    };
    let submitted = mismatch
        .into_handle()
        .try_into::<query_state::Submitted>()
        .unwrap();
    assert_eq!(submitted.id(), query_id);
}
