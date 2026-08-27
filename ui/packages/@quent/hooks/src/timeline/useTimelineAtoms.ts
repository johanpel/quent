// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useId, useMemo } from 'react';
import { useAtomValue, useSetAtom, useStore } from 'jotai';
import { useHydrateAtoms } from 'jotai/utils';
import {
  timelineDataMapAtom,
  zoomRangeAtom,
  debouncedZoomRangeAtom,
  timelineHoverAtom,
  timelinePointerAtom,
  startTimeMsAtom,
  bulkInitializedAtom,
  visibleEntriesAtom,
  longEntityDensityAtom,
  timelineCacheKey,
} from '../atoms/timeline';
import {
  getFsmTypeName,
  getResourceTypeName,
  type ResourceTimeline,
  type ZoomRange,
  type SingleTimelineResponse,
} from '@quent/utils';

// Record-based replacement for atomFamily(timelineDataAtom(key))
export function useTimelineData(key: string): SingleTimelineResponse | undefined {
  const map = useAtomValue(timelineDataMapAtom);
  return map[key];
}

function timelineMatchesActiveSpan(data: SingleTimelineResponse, activeSpan: ZoomRange): boolean {
  const tolerance = data.config.bin_duration;
  return (
    Math.abs(data.config.span.start - activeSpan.start) <= tolerance &&
    Math.abs(data.config.span.end - activeSpan.end) <= tolerance
  );
}

function useReturnedTimelineState(resourceId: string): {
  data: SingleTimelineResponse | undefined;
  isStale: boolean;
} {
  const timelineDataMap = useAtomValue(timelineDataMapAtom);
  const visibleEntries = useAtomValue(visibleEntriesAtom);
  const activeSpan = useAtomValue(debouncedZoomRangeAtom);
  const request = visibleEntries[resourceId];
  if (!request) {
    return { data: undefined, isStale: false };
  }
  const key = timelineCacheKey({
    resourceId,
    resourceTypeName: getResourceTypeName(request),
    fsmTypeName: getFsmTypeName(request),
  });
  const data = timelineDataMap[key];
  if (!data) {
    return { data: undefined, isStale: false };
  }
  return timelineMatchesActiveSpan(data, activeSpan)
    ? { data, isStale: false }
    : { data: undefined, isStale: true };
}

export function useReturnedTimelineNumBins(resourceId: string): number | undefined {
  const { data } = useReturnedTimelineState(resourceId);
  const numBins = Number(data?.config.num_bins);
  return Number.isInteger(numBins) && numBins > 0 ? numBins : undefined;
}

export function useReturnedTimelineIsStale(resourceId: string): boolean {
  return useReturnedTimelineState(resourceId).isStale;
}

/** Map resources with current cached timelines to whether any usage bin is nonzero. */
export function useReturnedTimelineActivity(): ReadonlyMap<string, boolean> {
  const timelineDataMap = useAtomValue(timelineDataMapAtom);
  const visibleEntries = useAtomValue(visibleEntriesAtom);
  const activeSpan = useAtomValue(debouncedZoomRangeAtom);

  return useMemo(() => {
    const activity = new Map<string, boolean>();
    for (const [resourceId, request] of Object.entries(visibleEntries)) {
      const key = timelineCacheKey({
        resourceId,
        resourceTypeName: getResourceTypeName(request),
        fsmTypeName: getFsmTypeName(request),
      });
      const response = timelineDataMap[key];
      if (!response) {
        continue;
      }
      if (timelineMatchesActiveSpan(response, activeSpan)) {
        activity.set(resourceId, timelineHasActivity(response.data));
      }
    }
    return activity;
  }, [activeSpan, timelineDataMap, visibleEntries]);
}

function timelineHasActivity(data: ResourceTimeline): boolean {
  if ('Binned' in data) {
    return Object.values(data.Binned.capacities_values).some(values =>
      values.some(value => value !== 0)
    );
  }
  return Object.values(data.BinnedByState.capacities_states_values).some(states =>
    Object.values(states).some(values => values.some(value => value !== 0))
  );
}

export const useZoomRange = () => useAtomValue(zoomRangeAtom);
export const useGetZoomRange = () => {
  const store = useStore();
  return useCallback(() => store.get(zoomRangeAtom), [store]);
};
export const useSetZoomRange = () => useSetAtom(zoomRangeAtom);
export function useReadZoomRange() {
  const store = useStore();
  return useCallback(() => store.get(zoomRangeAtom), [store]);
}
export const useDebouncedZoomRange = () => useAtomValue(debouncedZoomRangeAtom);
export const useSetDebouncedZoomRange = () => useSetAtom(debouncedZoomRangeAtom);
export const useLongEntityDensity = () => useAtomValue(longEntityDensityAtom);
export const useSetLongEntityDensity = () => useSetAtom(longEntityDensityAtom);
export const useTimelineHover = () => useAtomValue(timelineHoverAtom);
export const useSetTimelineHover = () => useSetAtom(timelineHoverAtom);
export const useTimelinePointerRatio = () => useAtomValue(timelinePointerAtom)?.ratio ?? null;
export function useTimelinePointerPublisher() {
  const ownerId = useId();
  const store = useStore();
  const setPointer = useSetAtom(timelinePointerAtom);
  const publish = useCallback(
    (ratio: number) => {
      setPointer({ ratio: Math.min(1, Math.max(0, ratio)), ownerId });
    },
    [ownerId, setPointer]
  );
  const clear = useCallback(() => {
    const ownedPointer = store.get(timelinePointerAtom);
    if (ownedPointer?.ownerId !== ownerId) {
      return;
    }
    const clearIfUnchanged = () => {
      if (store.get(timelinePointerAtom) === ownedPointer) {
        setPointer(null);
      }
    };
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(clearIfUnchanged);
    } else {
      setTimeout(clearIfUnchanged, 0);
    }
  }, [ownerId, setPointer, store]);

  useEffect(
    () => () => {
      if (store.get(timelinePointerAtom)?.ownerId === ownerId) {
        setPointer(null);
      }
    },
    [ownerId, setPointer, store]
  );

  return { publish, clear };
}
export const useStartTimeMs = () => useAtomValue(startTimeMsAtom);
export const useSetStartTimeMs = () => useSetAtom(startTimeMsAtom);
export const useBulkInitialized = () => useAtomValue(bulkInitializedAtom);
export const useSetBulkInitialized = () => useSetAtom(bulkInitializedAtom);
export const useVisibleEntries = () => useAtomValue(visibleEntriesAtom);
export const useSetVisibleEntries = () => useSetAtom(visibleEntriesAtom);

/**
 * Hydrates the timeline atoms with initial values synchronously during render.
 * Use this in the root component of a query view to initialize zoom and start time
 * before child components read them.
 */
export function useHydrateTimelineAtoms(params: {
  zoomRange: ZoomRange;
  debouncedZoomRange: ZoomRange;
  startTimeMs: number;
}): void {
  useHydrateAtoms([
    [zoomRangeAtom, params.zoomRange],
    [debouncedZoomRangeAtom, params.debouncedZoomRange],
    [startTimeMsAtom, params.startTimeMs],
  ]);
}
