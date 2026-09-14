// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { atom, useAtomValue } from 'jotai';
import { useHydrateAtoms } from 'jotai/utils';
import { atomFamily } from 'jotai-family';
import { createDeterministicColorResolver, type DeterministicColorResolver } from '@quent/utils';

export const COLOR_REGISTRY_KEYS = {
  OPERATOR_TYPES: 'operator-types',
  RESOURCE_TYPES: 'resource-types',
  FSM_TYPES: 'fsm-types',
} as const;

export type ColorRegistryKey = (typeof COLOR_REGISTRY_KEYS)[keyof typeof COLOR_REGISTRY_KEYS];
export type ColorRegistry = ReadonlyMap<ColorRegistryKey, ReadonlyMap<string, string>>;

const EMPTY_COLOR_MAP = new Map<string, string>();
const colorRegistryAtom = atom<ColorRegistry>(new Map());
const colorResolverAtomFamily = atomFamily((registryKey: ColorRegistryKey) =>
  atom(get =>
    createDeterministicColorResolver(get(colorRegistryAtom).get(registryKey) ?? EMPTY_COLOR_MAP)
  )
);

export function useColorResolver(registryKey: ColorRegistryKey): DeterministicColorResolver {
  return useAtomValue(colorResolverAtomFamily(registryKey));
}

/** Hydrates complete color maps before descendants read their resolvers. */
export function useHydrateColorRegistry(registry: ColorRegistry): void {
  useHydrateAtoms([[colorRegistryAtom, registry]]);
}
