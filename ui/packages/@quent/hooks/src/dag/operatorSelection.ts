// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { OperatorSelectionState } from '@quent/utils';

export function createEmptyOperatorSelectionState(): OperatorSelectionState {
  return {
    selections: new Map(),
  };
}

export function getSelectedOperatorIds(state: OperatorSelectionState): Set<string> {
  return new Set(
    Array.from(state.selections.values()).flatMap(selection => [...selection.operatorIds])
  );
}

function containsAll(container: ReadonlySet<string>, contained: ReadonlySet<string>): boolean {
  for (const id of contained) {
    if (!container.has(id)) {
      return false;
    }
  }
  return true;
}

export function addOperatorSelection(
  state: OperatorSelectionState,
  selectionId: string,
  label: string,
  operatorIds: Iterable<string>
): OperatorSelectionState {
  const selectedIds = new Set(operatorIds);
  selectedIds.add(selectionId);

  for (const [existingId, existing] of state.selections) {
    if (existingId !== selectionId && containsAll(existing.operatorIds, selectedIds)) {
      return state;
    }
  }

  const selections = new Map(state.selections);
  for (const [existingId, existing] of selections) {
    if (existingId !== selectionId && containsAll(selectedIds, existing.operatorIds)) {
      selections.delete(existingId);
    }
  }
  selections.set(selectionId, { label, operatorIds: selectedIds });

  return { selections };
}

export function removeOperatorSelection(
  state: OperatorSelectionState,
  selectionId: string
): OperatorSelectionState {
  const selections = new Map(state.selections);
  selections.delete(selectionId);

  return { selections };
}
