// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { useAtomValue, useSetAtom } from 'jotai';
import { useHydrateAtoms } from 'jotai/utils';
import { selectedPlanIdAtom } from '../atoms/dag';

export const useSelectedPlanId = () => useAtomValue(selectedPlanIdAtom);
export const useSetSelectedPlanId = () => useSetAtom(selectedPlanIdAtom);
export const useHydrateSelectedPlanId = (planId: string) => {
  useHydrateAtoms([[selectedPlanIdAtom, planId]]);
};
