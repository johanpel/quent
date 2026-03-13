import { atom } from 'jotai';
import { atomFamily } from 'jotai-family';
import type { SingleTimelineResponse } from '~quent/types/SingleTimelineResponse';
import type { TimelineRequest } from '~quent/types/TimelineRequest';
import type { OperatorFilter } from '~quent/types/OperatorFilter';
import type { ZoomRange } from '@/components/timeline/TimelineController';
import type { FiniteStateMachine } from '~quent/types/FiniteStateMachine';
import type { FsmEntityRef } from '~quent/types/FsmEntityRef';

/** Build a composite cache key for per-item timeline data */
export function timelineCacheKey(
  resourceId: string,
  resourceTypeName: string,
  operatorId: string | null = null
): string {
  return `${resourceId}|${resourceTypeName}|${operatorId ?? ''}`;
}

/** Per-item timeline data keyed by `timelineCacheKey(resourceId, resourceTypeName, operatorId)` */
export const timelineDataAtom = atomFamily(() =>
  atom<SingleTimelineResponse | undefined>(undefined)
);

/** Immediate zoom range — updated on every zoom gesture */
export const zoomRangeAtom = atom<ZoomRange>({ start: 0, end: 0 });

/** Debounced zoom range — settles after ZOOM_DEBOUNCE_MS, drives the bulk query */
export const debouncedZoomRangeAtom = atom<ZoomRange>({ start: 0, end: 0 });

/** Which timeline row is currently hovered (for tooltip display) */
export const hoveredTimelineIdAtom = atom<string | null>(null);

/**
 * Derived per-item hover check — only the two rows involved in a hover
 * change (old and new) re-render, not all rows.
 */
export const isTimelineHoveredAtom = atomFamily((itemId: string) =>
  atom(get => get(hoveredTimelineIdAtom) === itemId)
);

/** Start time in milliseconds — set once per query, never changes */
export const startTimeMsAtom = atom(0);

/** Flips to true after the first bulk fetch completes — gates individual fallback queries */
export const bulkInitializedAtom = atom(false);

/** Visible entries for bulk fetch — set in useEffect, read imperatively via store.get() */
export const visibleEntriesAtom = atom<Record<string, TimelineRequest<OperatorFilter>>>({});

/** When true, hides task annotation marks on timeline charts */
export const hideTasksAtom = atom(false);

/** Custom lane ordering: maps parent ID → ordered child IDs */
export const laneOrderAtom = atom<Map<string, string[]>>(new Map());

/**
 * Per-resource-group FSM type filter.
 * Key = item ID, value = selected FSM type name (null = all FSMs).
 * Missing key = default to first `used_by` entry.
 */
export const groupFsmFiltersAtom = atom<Map<string, string | null>>(new Map());

/** Engine ID for the current session */
export const engineIdAtom = atom('');

/** Query ID for the current session */
export const queryIdAtom = atom('');

/** Selected mark info for the detail panel */
export type SelectedMarkInfo = {
  fsm: FiniteStateMachine;
  activeStateName: string;
  screenX: number;
  screenY: number;
};
export const selectedMarkAtom = atom<SelectedMarkInfo | null>(null);

/** Tracked entity — its FSM is overlaid across all relevant timelines */
export type TrackedEntity = {
  entityRef: FsmEntityRef;
  fsm: FiniteStateMachine | null;
};
export const trackedEntityAtom = atom<TrackedEntity | null>(null);
