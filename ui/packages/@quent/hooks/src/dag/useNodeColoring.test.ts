// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { isOperatorGroupSelected } from './useNodeColoring';

describe('isOperatorGroupSelected', () => {
  it('deselects a parent visually when any covered child is deselected', () => {
    expect(isOperatorGroupSelected(new Set(['parent', 'right']), 'parent', ['left', 'right'])).toBe(
      false
    );
  });

  it('selects a parent visually when all covered children are selected', () => {
    expect(
      isOperatorGroupSelected(new Set(['parent', 'left', 'right']), 'parent', ['left', 'right'])
    ).toBe(true);
  });
});
