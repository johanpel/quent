// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { SelectedOperatorGroupData } from '@quent/utils';

export function findSelectedOperatorData(
  current: ReadonlyMap<string, SelectedOperatorGroupData>,
  operatorId: string
): SelectedOperatorGroupData | undefined {
  const direct = current.get(operatorId);
  if (direct) {
    return direct;
  }
  for (const data of current.values()) {
    if (data.nodeId === operatorId) {
      return data;
    }
    const related = data.relatedOperators?.find(operator => operator.nodeId === operatorId);
    if (related) {
      return related;
    }
  }
  return undefined;
}

export function upsertSelectedOperatorData(
  current: ReadonlyMap<string, SelectedOperatorGroupData>,
  selectionId: string,
  data: SelectedOperatorGroupData
): Map<string, SelectedOperatorGroupData> {
  const next = new Map(current);
  next.set(selectionId, data);
  return next;
}

export function removeSelectedOperatorData(
  current: ReadonlyMap<string, SelectedOperatorGroupData>,
  selectionId: string
): ReadonlyMap<string, SelectedOperatorGroupData> {
  if (!current.has(selectionId)) {
    return current;
  }
  const next = new Map(current);
  next.delete(selectionId);
  return next;
}
