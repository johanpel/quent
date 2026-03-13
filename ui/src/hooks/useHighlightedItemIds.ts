import { useMemo } from 'react';
import { useAtomValue } from 'jotai';
import { hoveredWorkerIdAtom } from '@/atoms/dag';
import { trackedEntityAtom } from '@/atoms/timeline';
import { TreeTableItem } from '@/components/resource-tree/types';
import { operatorTimelineRowId } from '@/components/operator-timeline';

/**
 * Returns the set of item IDs that should be highlighted in the resource tree.
 * Sources: hovered worker subtree, tracked entity resource rows.
 */
export function useHighlightedItemIds(rootItem: TreeTableItem): Set<string> | undefined {
  const hoveredWorkerId = useAtomValue(hoveredWorkerIdAtom);
  const trackedEntity = useAtomValue(trackedEntityAtom);

  return useMemo(() => {
    const ids = new Set<string>();

    // Highlight subtree of hovered worker
    if (hoveredWorkerId) {
      function collectSubtree(items: TreeTableItem[]) {
        for (const item of items) {
          ids.add(item.id);
          if (item.children) collectSubtree(item.children);
        }
      }

      function find(items: TreeTableItem[]): boolean {
        for (const item of items) {
          if (item.id === hoveredWorkerId) {
            collectSubtree([item]);
            ids.add(operatorTimelineRowId(hoveredWorkerId));
            return true;
          }
          if (item.children && find(item.children)) return true;
        }
        return false;
      }

      find([rootItem]);
    }

    // Highlight rows containing tracked entity marks
    if (trackedEntity?.fsm) {
      for (const t of trackedEntity.fsm.transitions) {
        for (const u of t.usages) {
          ids.add(u.resource);
        }
      }
    }

    return ids.size > 0 ? ids : undefined;
  }, [hoveredWorkerId, trackedEntity, rootItem]);
}
