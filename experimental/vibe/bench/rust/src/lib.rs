// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Generated Quent instrumentation used by the Rust benchmark.

#[allow(unused)]
pub mod instrumentation {
    include!(concat!(env!("OUT_DIR"), "/instrumentation.rs"));
}

#[cfg(test)]
mod tests {
    use super::instrumentation::{Context, Entity, InstrumentationLatency, Noop};

    #[test]
    fn tick_is_repeatable() {
        let context = Context::<InstrumentationLatency>::try_new(Noop).unwrap();
        let entity = context.observer::<Entity>().handle();
        entity.tick().unwrap();
        entity.tick().unwrap();
    }
}
