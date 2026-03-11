import { Column, TreeTable } from '@/components/ui/tree-table';
import { useCallback, useMemo, useState } from 'react';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useHydrateAtoms } from 'jotai/utils';
import { useHighlightedItemIds } from '@/hooks/useHighlightedItemIds';
import { ResourceTree } from '~quent/types/ResourceTree';
import { TimelineController } from './timeline/TimelineController';
import { collectResourceTypesFromTree } from '@/lib/resource.utils';
import { EntityRefKey } from '@/types';
import { TreeTableItem } from './resource-tree/types';
import { ResourceColumn } from './resource-tree/ResourceColumn';
import { UsageColumn } from './resource-tree/UsageColumn';
import { QueryBundle } from '~quent/types/QueryBundle';
import type { EntityRef } from '~quent/types/EntityRef';
import { fetchSingleTimeline, DEFAULT_STALE_TIME } from '@/services/api';
import type { SingleTimelineRequest } from '~quent/types/SingleTimelineRequest';
import type { QueryFilter } from '~quent/types/QueryFilter';
import type { OperatorFilter } from '~quent/types/OperatorFilter';
import { transformResourceTree, getAdaptiveNumBins, nanosToMs } from '@/lib/timeline.utils';
import { useExpandedIds } from '@/hooks/useExpandedIds';
import { useBulkTimelines } from '@/hooks/useBulkTimelines';
import { zoomRangeAtom, debouncedZoomRangeAtom, startTimeMsAtom, laneOrderAtom, groupFsmFiltersAtom } from '@/atoms/timeline';
import { useAtom, useAtomValue } from 'jotai';
import { TimelineToolbar } from './timeline/TimelineToolbar';

function getRootResourceGroupId(resourceTree: ResourceTree<EntityRef>): string | null {
  if (!('ResourceGroup' in resourceTree)) return null;
  const [, entityId] = Object.entries(resourceTree.ResourceGroup.id)[0] as [EntityRefKey, string];
  return entityId;
}

interface QueryResourceTreeProps {
  engineId: string;
  queryBundle: QueryBundle<EntityRef>;
}

export function QueryResourceTree(props: QueryResourceTreeProps) {
  return <QueryResourceTreeContent {...props} />;
}

function QueryResourceTreeContent({ queryBundle, engineId }: QueryResourceTreeProps) {
  const { entities, resource_tree: resourceTree } = queryBundle;
  const [selectedTypes, setSelectedTypes] = useState<Map<string, string>>(new Map());
  const [laneOrder, setLaneOrder] = useAtom(laneOrderAtom);

  const startTime = queryBundle.start_time_unix_ns;
  const durationSeconds = queryBundle.duration_s;
  const startTimeMs = useMemo(() => nanosToMs(startTime), [startTime]);

  useHydrateAtoms([
    [zoomRangeAtom, { start: 0, end: durationSeconds }],
    [debouncedZoomRangeAtom, { start: 0, end: durationSeconds }],
    [startTimeMsAtom, startTimeMs],
  ]);

  const groupFsmFilters = useAtomValue(groupFsmFiltersAtom);

  const rootItem = useMemo(
    () => transformResourceTree(entities, resourceTree, laneOrder),
    [resourceTree, entities, laneOrder]
  );

  const highlightedItemIds = useHighlightedItemIds(rootItem);

  const resourceTypeOptions = useMemo(() => collectResourceTypesFromTree([rootItem]), [rootItem]);

  const [rootResourceType, setRootResourceType] = useState<string>(resourceTypeOptions[0] || '');

  const rootResourceGroupId = useMemo(() => getRootResourceGroupId(resourceTree), [resourceTree]);

  const { expandedIds, handleExpandChange } = useExpandedIds(rootItem.id);

  const handleReorder = useCallback(
    (parentId: string | null, fromId: string, toId: string) => {
      if (!parentId) return;
      // Find the parent's current children order.
      const findChildren = (item: TreeTableItem): TreeTableItem[] | undefined => {
        if (item.id === parentId) return item.children;
        for (const child of item.children ?? []) {
          const found = findChildren(child);
          if (found) return found;
        }
        return undefined;
      };
      const children = findChildren(rootItem);
      if (!children) return;
      const ids = children.map(c => c.id);
      const fromIdx = ids.indexOf(fromId);
      const toIdx = ids.indexOf(toId);
      if (fromIdx === -1 || toIdx === -1) return;
      ids.splice(fromIdx, 1);
      ids.splice(toIdx, 0, fromId);
      setLaneOrder(prev => new Map(prev).set(parentId, ids));
    },
    [rootItem, setLaneOrder]
  );

  const { handleZoomChange, handleExpand } = useBulkTimelines({
    engineId,
    queryId: queryBundle.query_id,
    rootItem,
    expandedIds,
    selectedTypes,
    entities,
  });

  const onExpandChange = useCallback(
    (itemId: string, isExpanded: boolean) => {
      handleExpandChange(itemId, isExpanded);
      handleExpand(itemId, isExpanded);
    },
    [handleExpandChange, handleExpand]
  );

  const { data: rootTimelineData } = useQuery({
    queryKey: [
      'resourceGroupTimeline',
      'root',
      engineId,
      queryBundle.query_id,
      rootResourceGroupId,
      durationSeconds,
      rootResourceType,
      groupFsmFilters.get(rootResourceGroupId!),
    ],
    queryFn: () => {
      const rootFsmFilter = rootResourceGroupId ? groupFsmFilters.get(rootResourceGroupId) : undefined;
      const request: SingleTimelineRequest<QueryFilter, OperatorFilter> = {
        entry: {
          ResourceGroup: {
            resource_group_id: rootResourceGroupId!,
            resource_type_name: rootResourceType,
            long_entities_threshold_s: null,
            entity_filter: { entity_type_name: rootFsmFilter ?? null },
            app_params: { operator_id: null },
            config: {
              num_bins: getAdaptiveNumBins(),
              start: 0,
              end: durationSeconds,
            },
          },
        },
        app_params: { query_id: queryBundle.query_id },
      };
      return fetchSingleTimeline(engineId, request, durationSeconds);
    },
    staleTime: DEFAULT_STALE_TIME,
    enabled: rootResourceGroupId != null && !!rootResourceType,
    placeholderData: keepPreviousData,
  });

  const treeData = useMemo(() => [rootItem], [rootItem]);

  const columns = useMemo(() => {
    return [
      {
        key: 'resource',
        label: 'Resource',
        widthIndex: 0,
        isFirst: true,
        render: ({ item, isExpanded }: { item: TreeTableItem; level: number; isExpanded: boolean }) => (
          <ResourceColumn
            item={item}
            isExpanded={isExpanded}
            selectedType={selectedTypes.get(item.id) || item.availableResourceTypes?.[0] || ''}
            onTypeChange={(itemId, newType) => {
              setSelectedTypes(prev => new Map(prev).set(itemId, newType));
              if (itemId === rootItem.id) {
                setRootResourceType(newType);
              }
            }}
            entities={entities}
          />
        ),
      },
      {
        key: 'usage',
        label: 'Usage',
        widthIndex: 1,
        subHeaderContent: (
          <div className="h-full overflow-hidden flex items-center py-2">
            <TimelineController
              startTime={startTime}
              durationSeconds={durationSeconds}
              timelineData={rootTimelineData}
              onZoomChange={handleZoomChange}
            />
          </div>
        ),
        render: ({ item, isExpanded }: { item: TreeTableItem; isExpanded: boolean }) => (
          <UsageColumn
            item={item}
            isExpanded={isExpanded}
            engineId={engineId}
            queryBundle={queryBundle}
            selectedTypes={selectedTypes}
            startTime={startTime}
            durationSeconds={durationSeconds}
          />
        ),
      },
    ] satisfies Column<TreeTableItem>[];
  }, [
    startTime,
    durationSeconds,
    rootTimelineData,
    selectedTypes,
    rootItem,
    engineId,
    queryBundle,
    handleZoomChange,
  ]);

  return (
    <div className="flex flex-col h-full w-full">
      <TimelineToolbar durationSeconds={durationSeconds} />
      <div className="flex-1 min-h-0">
        <TreeTable<TreeTableItem>
          data={treeData}
          columns={columns}
          initialSelectedItemId={rootItem.id}
          columnWidths={[275, 'auto']}
          onExpandChange={onExpandChange}
          onReorder={handleReorder}
          highlightedItemIds={highlightedItemIds}
        />
      </div>
    </div>
  );
}
