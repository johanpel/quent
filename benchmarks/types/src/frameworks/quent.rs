// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Quent benchmark options shared by the runner and implementations.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Selects the channel used by Quent benchmark cases.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Channel {
    Tokio,
    PerThread,
}

impl Channel {
    /// Returns the component included in the benchmark implementation name.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Tokio => None,
            Self::PerThread => Some("pt"),
        }
    }

    /// Returns the Cargo features required by the benchmark implementation.
    pub fn features(self) -> &'static [&'static str] {
        match self {
            Self::Tokio => &[],
            Self::PerThread => &["channel-per-thread"],
        }
    }
}

/// Selects the clock used by Quent instrumentation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Clock {
    Std,
    Quanta,
}

impl Clock {
    /// Returns the component included in the benchmark implementation name.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Self::Std => None,
            Self::Quanta => Some("quanta"),
        }
    }

    /// Returns the Cargo features required by the benchmark implementation.
    pub fn features(self) -> &'static [&'static str] {
        match self {
            Self::Std => &[],
            Self::Quanta => &["clock-quanta"],
        }
    }
}

/// Selects the exporter used by Quent benchmark cases.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Exporter {
    Noop,
    Ndjson,
    Msgpack,
    Postcard,
}

impl Exporter {
    /// Returns the value accepted by the Quent implementation executable.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::Ndjson => "ndjson",
            Self::Msgpack => "msgpack",
            Self::Postcard => "postcard",
        }
    }
}

/// Returns the report name for a Quent channel and clock combination.
pub fn variant_label(channel: Channel, clock: Clock) -> String {
    std::iter::once("quent")
        .chain(channel.label())
        .chain(clock.label())
        .collect::<Vec<_>>()
        .join("-")
}
