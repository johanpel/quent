// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint and metadata payload conversion.
//!
//! serde-saphyr has already deserialized each payload into a JSON value. A
//! null carries no data, a string passes through verbatim, and anything else
//! becomes canonical (compact, key-sorted) JSON.

use serde_json::Value;

/// Convert one annotation payload to constraint/metadata data.
///
/// `None` for a null payload, `Some(data)` otherwise.
pub(crate) fn payload(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        _ => Some(serde_json::to_string(value).expect("JSON values serialize")),
    }
}
