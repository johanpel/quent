// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { Operator } from './types';
import type { OperatorSelectionInput } from './operatorTypes';

export interface ResolvedOperatorSelectionCandidates<
  Selection extends OperatorSelectionInput = OperatorSelectionInput,
> {
  selections: Selection[];
  unresolvedOperatorIds: ReadonlySet<string>;
}

export function getOperatorDisplayLabel(operator: Operator): string {
  return operator.instance_name ?? operator.operator_type_name ?? operator.id;
}

export function buildRelatedOperatorIdsById(
  operators: readonly Operator[],
  rootOperatorIds: Iterable<string> = operators.map(operator => operator.id)
): Map<string, string[]> {
  const childrenByParentId = new Map<string, string[]>();
  for (const operator of operators) {
    for (const parentId of operator.parent_operator_ids ?? []) {
      const children = childrenByParentId.get(parentId) ?? [];
      children.push(operator.id);
      childrenByParentId.set(parentId, children);
    }
  }

  const relatedById = new Map<string, string[]>();
  for (const operatorId of rootOperatorIds) {
    const related = new Set<string>();
    const stack = [...(childrenByParentId.get(operatorId) ?? [])];
    while (stack.length > 0) {
      const childId = stack.pop()!;
      if (childId === operatorId || related.has(childId)) {
        continue;
      }
      related.add(childId);
      stack.push(...(childrenByParentId.get(childId) ?? []));
    }
    relatedById.set(operatorId, [...related].sort());
  }

  return relatedById;
}

export function resolveOperatorSelectionCandidates<Selection extends OperatorSelectionInput>(
  candidates: readonly Selection[],
  selectedOperatorIds: Iterable<string>
): ResolvedOperatorSelectionCandidates<Selection> {
  const unresolvedOperatorIds = new Set(selectedOperatorIds);
  const orderedCandidates = candidates
    .map((selection, index) => ({ selection, index }))
    .sort(
      (left, right) =>
        right.selection.operatorIds.size - left.selection.operatorIds.size ||
        left.index - right.index
    );
  const selections: Selection[] = [];

  for (const { selection } of orderedCandidates) {
    if (![...selection.operatorIds].every(id => unresolvedOperatorIds.has(id))) {
      continue;
    }
    selections.push(selection);
    for (const id of selection.operatorIds) {
      unresolvedOperatorIds.delete(id);
    }
  }

  return { selections, unresolvedOperatorIds };
}

export function resolveOperatorSelections(
  operators: readonly Operator[],
  selectedOperatorIds: Iterable<string>
): OperatorSelectionInput[] {
  const selectedIds = [...new Set(selectedOperatorIds)];
  const operatorsById = new Map(operators.map(operator => [operator.id, operator]));
  const relatedById = buildRelatedOperatorIdsById(operators, selectedIds);
  const candidates = selectedIds.flatMap(id => {
    const operator = operatorsById.get(id);
    if (!operator) {
      return [];
    }
    return [
      {
        operator,
        operatorIds: new Set([id, ...(relatedById.get(id) ?? [])]),
      },
    ];
  });
  const resolved = resolveOperatorSelectionCandidates(
    candidates.map(candidate => ({
      selectionId: candidate.operator.id,
      label: getOperatorDisplayLabel(candidate.operator),
      operatorIds: candidate.operatorIds,
    })),
    selectedIds
  );
  const selections: OperatorSelectionInput[] = [...resolved.selections];
  const remainingIds = new Set(resolved.unresolvedOperatorIds);

  for (const id of selectedIds) {
    if (!remainingIds.delete(id)) {
      continue;
    }
    const operator = operatorsById.get(id);
    selections.push({
      selectionId: id,
      label: operator ? getOperatorDisplayLabel(operator) : id,
      operatorIds: new Set([id]),
    });
  }

  return selections;
}
