// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { InspectedNodeData } from '@quent/utils';

export function upsertInspectedNodeData(
  current: ReadonlyMap<string, InspectedNodeData>,
  selectionId: string,
  data: InspectedNodeData
): Map<string, InspectedNodeData> {
  const next = new Map(current);
  next.set(selectionId, data);
  return next;
}

export function removeInspectedNodeData(
  current: ReadonlyMap<string, InspectedNodeData>,
  selectionId: string
): ReadonlyMap<string, InspectedNodeData> {
  if (!current.has(selectionId)) {
    return current;
  }
  const next = new Map(current);
  next.delete(selectionId);
  return next;
}
