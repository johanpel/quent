// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { FsmTypeSelection, ResourceGroupTypeDecl, ResourceTypeDecl } from './types';

/** Whether `choices` contains the named FSM type ("All" never matches). */
function hasFsmType(choices: readonly FsmTypeSelection[], type: string): boolean {
  return choices.some(choice => choice !== 'All' && choice.Type === type);
}

/** Return the declared FSM choices available for a resource, in group preference order. */
export function resourceFsmChoices(
  declaration: ResourceTypeDecl | undefined,
  groupDeclaration?: ResourceGroupTypeDecl
): FsmTypeSelection[] {
  const choices = declaration?.display_order ?? [];
  if (!groupDeclaration?.display_order.length) {
    return choices;
  }
  const groupChoices = groupDeclaration.display_order.filter(
    selection => selection === 'All' || hasFsmType(choices, selection.Type)
  );
  return [
    ...groupChoices,
    ...choices.filter(choice => choice !== 'All' && !hasFsmType(groupChoices, choice.Type)),
  ];
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
  const fsmChoices = choices.filter(choice => choice !== 'All');
  // With a single FSM, "All" is just that FSM's aggregate; show the FSM itself.
  if (fsmChoices.length === 1) {
    return fsmChoices[0]!.Type;
  }
  const initial = choices[0];
  return initial && initial !== 'All' ? initial.Type : null;
}
