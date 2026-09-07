#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

tutorial_log="$(mktemp)"
trap 'rm -f "$tutorial_log"' EXIT

mdbook build crates/yaml/tutorial 2>&1 | tee "$tutorial_log"

# mdBook reports missing includes as errors without returning a failure status.
if grep -q '^ERROR ' "$tutorial_log"; then
  exit 1
fi
