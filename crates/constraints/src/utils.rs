// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Shared helpers for constraint implementations.

use std::fmt::Display;

/// Format items as a bulleted, newline-separated list.
pub fn bullet_list<T: Display>(items: &[T]) -> String {
    items
        .iter()
        .map(|item| format!("  - {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collapse collected errors into a result.
///
/// A single error is returned as-is, several are wrapped through `multiple`.
pub fn collapse<E>(mut errors: Vec<E>, multiple: impl FnOnce(Vec<E>) -> E) -> Result<(), E> {
    match errors.len() {
        0 => Ok(()),
        1 => Err(errors.pop().unwrap()),
        _ => Err(multiple(errors)),
    }
}
