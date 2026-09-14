// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { PropsWithChildren } from 'react';
import { renderHook } from '@testing-library/react';
import { Provider } from 'jotai';
import { describe, expect, it } from 'vitest';
import { getDeterministicColor } from '@quent/utils';
import {
  COLOR_REGISTRY_KEYS,
  useColorResolver,
  useHydrateColorRegistry,
  type ColorRegistry,
} from './colorRegistry';

function createWrapper(registry: ColorRegistry) {
  function Hydrator({ children }: PropsWithChildren) {
    useHydrateColorRegistry(registry);
    return children;
  }
  return function Wrapper({ children }: PropsWithChildren) {
    return (
      <Provider>
        <Hydrator>{children}</Hydrator>
      </Provider>
    );
  };
}

describe('color registry', () => {
  it('hydrates namespace resolvers before their first read', () => {
    const registry: ColorRegistry = new Map([
      [COLOR_REGISTRY_KEYS.OPERATOR_TYPES, new Map([['project', '#123456']])],
    ]);
    const { result } = renderHook(() => useColorResolver(COLOR_REGISTRY_KEYS.OPERATOR_TYPES), {
      wrapper: createWrapper(registry),
    });

    expect(result.current('Project')).toBe('#123456');
  });

  it('keeps assignments independent across registry keys', () => {
    const registry: ColorRegistry = new Map([
      [COLOR_REGISTRY_KEYS.OPERATOR_TYPES, new Map([['shared', '#111111']])],
      [COLOR_REGISTRY_KEYS.RESOURCE_TYPES, new Map([['shared', '#222222']])],
    ]);
    const { result } = renderHook(
      () => ({
        operator: useColorResolver(COLOR_REGISTRY_KEYS.OPERATOR_TYPES),
        resource: useColorResolver(COLOR_REGISTRY_KEYS.RESOURCE_TYPES),
      }),
      { wrapper: createWrapper(registry) }
    );

    expect(result.current.operator('shared')).toBe('#111111');
    expect(result.current.resource('shared')).toBe('#222222');
  });

  it('isolates registries in separate query providers', () => {
    const first: ColorRegistry = new Map([
      [COLOR_REGISTRY_KEYS.OPERATOR_TYPES, new Map([['project', '#111111']])],
    ]);
    const second: ColorRegistry = new Map([
      [COLOR_REGISTRY_KEYS.OPERATOR_TYPES, new Map([['project', '#222222']])],
    ]);
    const firstResult = renderHook(() => useColorResolver(COLOR_REGISTRY_KEYS.OPERATOR_TYPES), {
      wrapper: createWrapper(first),
    });
    const secondResult = renderHook(() => useColorResolver(COLOR_REGISTRY_KEYS.OPERATOR_TYPES), {
      wrapper: createWrapper(second),
    });

    expect(firstResult.result.current('project')).toBe('#111111');
    expect(secondResult.result.current('project')).toBe('#222222');
  });

  it('falls back when a namespace or key was not hydrated', () => {
    const { result } = renderHook(() => useColorResolver(COLOR_REGISTRY_KEYS.FSM_TYPES), {
      wrapper: createWrapper(new Map()),
    });

    expect(result.current('Task')).toBe(getDeterministicColor('Task'));
  });
});
