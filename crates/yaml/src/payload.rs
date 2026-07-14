// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Constraint and metadata payload conversion.
//!
//! A null payload carries no data, a string passes through verbatim, and any
//! other value becomes canonical (compact, key-sorted) JSON.

use serde_norway::Value;

/// Convert one annotation payload to constraint/metadata data.
///
/// `Ok(None)` for a null payload, `Ok(Some(data))` otherwise, `Err(reason)`
/// for a value with no JSON representation.
pub(crate) fn payload(value: &Value) -> Result<Option<String>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => {
            let json = to_json(value)?;
            Ok(Some(
                serde_json::to_string(&json).expect("JSON values serialize"),
            ))
        }
    }
}

fn to_json(value: &Value) -> Result<serde_json::Value, String> {
    Ok(match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Number(n) => number_to_json(n)?,
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Sequence(items) => {
            serde_json::Value::Array(items.iter().map(to_json).collect::<Result<_, _>>()?)
        }
        Value::Mapping(map) => {
            let mut object = serde_json::Map::new();
            for (key, val) in map {
                object.insert(key_to_string(key)?, to_json(val)?);
            }
            serde_json::Value::Object(object)
        }
        Value::Tagged(_) => return Err("YAML tags are not supported in payloads".to_string()),
    })
}

fn number_to_json(n: &serde_norway::Number) -> Result<serde_json::Value, String> {
    if let Some(i) = n.as_i64() {
        Ok(i.into())
    } else if let Some(u) = n.as_u64() {
        Ok(u.into())
    } else if let Some(f) = n.as_f64().and_then(serde_json::Number::from_f64) {
        Ok(serde_json::Value::Number(f))
    } else {
        Err(format!("number `{n}` has no JSON representation; quote it"))
    }
}

/// JSON object keys are strings, so a scalar key becomes its text.
fn key_to_string(key: &Value) -> Result<String, String> {
    match key {
        Value::String(s) => Ok(s.clone()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok("~".to_string()),
        _ => Err("payload mapping keys must be scalars".to_string()),
    }
}
