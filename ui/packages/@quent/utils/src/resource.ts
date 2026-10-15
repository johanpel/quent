// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { FsmTypeSelection, ResourceGroupTypeDecl, ResourceTypeDecl } from './types';

/** The FSM selection that aggregates over every FSM of a resource. */
export const ALL_FSMS = 'All' satisfies FsmTypeSelection;

/** Whether `choices` contains the named FSM type ("All" never matches). */
function hasFsmType(choices: readonly FsmTypeSelection[], type: string): boolean {
  return choices.some(choice => choice !== ALL_FSMS && choice.Type === type);
}

/** Merge the resource's declared FSM choices with the group's preferred order. */
function orderedFsmChoices(
  declaration: ResourceTypeDecl | undefined,
  groupDeclaration?: ResourceGroupTypeDecl
): FsmTypeSelection[] {
  const choices = declaration?.display_order ?? [];
  if (!groupDeclaration?.display_order.length) {
    return choices;
  }
  const groupChoices = groupDeclaration.display_order.filter(
    selection => selection === ALL_FSMS || hasFsmType(choices, selection.Type)
  );
  return [
    ...groupChoices,
    ...choices.filter(choice => choice !== ALL_FSMS && !hasFsmType(groupChoices, choice.Type)),
  ];
}

/**
 * Return the declared FSM choices available for a resource, in group preference order.
 * With a single FSM, "All" is just that FSM's aggregate, so it is not offered.
 */
export function resourceFsmChoices(
  declaration: ResourceTypeDecl | undefined,
  groupDeclaration?: ResourceGroupTypeDecl
): FsmTypeSelection[] {
  const choices = orderedFsmChoices(declaration, groupDeclaration);
  const fsmChoices = choices.filter(choice => choice !== ALL_FSMS);
  return fsmChoices.length === 1 ? fsmChoices : choices;
}

/** Resolve the FSM filter, preserving explicit selections including null (all FSMs). */
export function resolveResourceFsmType({
  resourceType,
  selectedFsmType,
  resourceGroupType,
}: {
  resourceType?: ResourceTypeDecl;
  selectedFsmType?: string | null;
  resourceGroupType?: ResourceGroupTypeDecl;
}): string | null {
  if (selectedFsmType !== undefined) {
    return selectedFsmType;
  }
  const choices = resourceFsmChoices(resourceType, resourceGroupType);
  const initial = choices[0];
  return initial && initial !== ALL_FSMS ? initial.Type : null;
}
