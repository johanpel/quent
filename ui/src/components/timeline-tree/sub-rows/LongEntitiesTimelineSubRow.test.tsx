// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { LONG_ENTITIES_ROW_TYPE, longEntitiesRowId, type TreeTableItem } from '@quent/components';
import { EntityTypeKey, type EntityRef, type QueryBundle } from '@quent/utils';
import { createLongEntitiesTimelineSubRow } from './LongEntitiesTimelineSubRow';

const RESOURCE_ID = 'resource-1';
const rootItem: TreeTableItem = {
  id: 'root',
  type: EntityTypeKey.ResourceGroup,
  entity: {} as TreeTableItem['entity'],
  children: [
    {
      id: RESOURCE_ID,
      type: EntityTypeKey.Resource,
      entity: {} as TreeTableItem['entity'],
    },
  ],
};
const queryBundle = {
  query_id: 'query-1',
  duration_s: 1,
  entities: { fsm_types: {} },
} as unknown as QueryBundle<EntityRef>;

function createSubRow(resourceActivity?: ReadonlyMap<string, boolean>) {
  return createLongEntitiesTimelineSubRow({
    engineId: 'engine-1',
    queryBundle,
    isDark: false,
    resourceActivity,
  });
}

describe('createLongEntitiesTimelineSubRow', () => {
  it('keeps the entity lane while resource activity is unknown', () => {
    const tree = createSubRow().injectRows(rootItem);

    expect(tree.children?.map(child => child.id)).toEqual([
      RESOURCE_ID,
      longEntitiesRowId(RESOURCE_ID),
    ]);
  });

  it('removes the entity lane for an inactive resource', () => {
    const tree = createSubRow(new Map([[RESOURCE_ID, false]])).injectRows(rootItem);

    expect(tree.children?.map(child => child.id)).toEqual([RESOURCE_ID]);
    expect(tree.children?.some(child => child.type === LONG_ENTITIES_ROW_TYPE)).toBe(false);
  });

  it('keeps the entity lane for an active resource', () => {
    const tree = createSubRow(new Map([[RESOURCE_ID, true]])).injectRows(rootItem);

    expect(tree.children?.some(child => child.type === LONG_ENTITIES_ROW_TYPE)).toBe(true);
  });
});
