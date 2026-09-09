// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import type { InspectedNodeData } from '@quent/utils';
import { removeInspectedNodeData, upsertInspectedNodeData } from './inspectedNodeData';

const scan: InspectedNodeData = {
  nodeId: 'scan',
  label: 'Scan',
  operationType: 'scan',
  statistics: [],
};

const join: InspectedNodeData = {
  nodeId: 'join',
  label: 'Join',
  operationType: 'join',
  statistics: [],
};

describe('inspected node data', () => {
  it('adds a second operator without replacing the first', () => {
    const first = upsertInspectedNodeData(new Map(), 'scan', scan);
    const second = upsertInspectedNodeData(first, 'join', join);

    expect([...second.keys()]).toEqual(['scan', 'join']);
    expect(second.get('scan')).toEqual(scan);
    expect(second.get('join')).toEqual(join);
  });

  it('keys inspected data by selection ID instead of node ID', () => {
    const result = upsertInspectedNodeData(new Map(), 'logical-join', join);

    expect(result).toEqual(new Map([['logical-join', join]]));
  });

  it('removes only the requested operator', () => {
    const both = upsertInspectedNodeData(
      upsertInspectedNodeData(new Map(), 'scan', scan),
      'join',
      join
    );

    expect(removeInspectedNodeData(both, 'scan')).toEqual(new Map([['join', join]]));
  });
});
