// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import type { PropsWithChildren } from 'react';
import { act, renderHook } from '@testing-library/react';
import { Provider, createStore } from 'jotai';
import { describe, expect, it, vi } from 'vitest';
import type { OperatorFilter, SingleTimelineResponse, TimelineRequest } from '@quent/utils';
import {
  debouncedZoomRangeAtom,
  timelineCacheKey,
  timelineDataMapAtom,
  timelinePointerAtom,
  visibleEntriesAtom,
  zoomRangeAtom,
} from '../atoms/timeline';
import {
  useGetZoomRange,
  useReturnedTimelineActivity,
  useReturnedTimelineIsStale,
  useReturnedTimelineNumBins,
  useTimelinePointerPublisher,
  useTimelinePointerRatio,
} from './useTimelineAtoms';

function makeTimelineRequest(): TimelineRequest<OperatorFilter> {
  return {
    Resource: {
      resource_id: 'resource-1',
      long_entities_threshold_s: null,
      entity_filter: { entity_type_name: 'fsm-1' },
      application: { operator_ids: [] },
      config: { num_bins: 2, start: 0, end: 1 },
    },
  };
}

function makeTimelineResponse(
  data: SingleTimelineResponse['data'],
  span = { start: 0, end: 1 }
): SingleTimelineResponse {
  return {
    config: { span, bin_duration: 0.5, num_bins: 2n },
    data,
  };
}

function timelineCacheKeyForTest(): string {
  return timelineCacheKey({
    resourceId: 'resource-1',
    resourceTypeName: '',
    fsmTypeName: 'fsm-1',
  });
}

describe('useGetZoomRange', () => {
  it('reads the latest zoom without subscribing the chart to zoom updates', () => {
    const store = createStore();
    store.set(zoomRangeAtom, { start: 0, end: 100 });
    let renderCount = 0;
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );
    const { result } = renderHook(
      () => {
        renderCount += 1;
        return useGetZoomRange();
      },
      { wrapper }
    );

    act(() => store.set(zoomRangeAtom, { start: 25, end: 75 }));

    expect(renderCount).toBe(1);
    expect(result.current()).toEqual({ start: 25, end: 75 });
  });
});

describe('timeline pointer', () => {
  it('publishes a ratio and only clears the current owner', () => {
    const store = createStore();
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    });

    try {
      const { result } = renderHook(
        () => ({
          first: useTimelinePointerPublisher(),
          second: useTimelinePointerPublisher(),
          ratio: useTimelinePointerRatio(),
        }),
        { wrapper }
      );

      act(() => result.current.first.publish(0.25));
      expect(result.current.ratio).toBe(0.25);
      act(() => result.current.first.clear());
      act(() => result.current.first.publish(0.5));
      act(() => frames.shift()?.(0));
      expect(result.current.ratio).toBe(0.5);
      act(() => result.current.second.publish(0.75));
      act(() => result.current.first.clear());
      expect(result.current.ratio).toBe(0.75);
      act(() => result.current.second.clear());
      act(() => frames.shift()?.(0));
      expect(result.current.ratio).toBeNull();
      expect(store.get(timelinePointerAtom)).toBeNull();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe('useReturnedTimelineNumBins', () => {
  it('reads the returned bin count for the visible resource request', () => {
    const store = createStore();
    const request: TimelineRequest<OperatorFilter> = {
      Resource: {
        resource_id: 'resource-1',
        long_entities_threshold_s: null,
        entity_filter: { entity_type_name: 'fsm-1' },
        application: { operator_ids: [] },
        config: { num_bins: 200, start: 0, end: 1 },
      },
    };
    const response: SingleTimelineResponse = {
      config: {
        span: { start: -0.001, end: 1.001 },
        bin_duration: 0.0025,
        num_bins: 400n,
      },
      data: {} as SingleTimelineResponse['data'],
    };
    const cacheKey = timelineCacheKey({
      resourceId: 'resource-1',
      resourceTypeName: '',
      fsmTypeName: 'fsm-1',
    });
    store.set(visibleEntriesAtom, { 'resource-1': request });
    store.set(timelineDataMapAtom, { [cacheKey]: response });
    store.set(debouncedZoomRangeAtom, { start: 0, end: 1 });
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );

    const { result } = renderHook(() => useReturnedTimelineNumBins('resource-1'), { wrapper });

    expect(result.current).toBe(400);
  });

  it('returns undefined when no response is cached', () => {
    const store = createStore();
    const request: TimelineRequest<OperatorFilter> = {
      Resource: {
        resource_id: 'resource-1',
        long_entities_threshold_s: null,
        entity_filter: { entity_type_name: 'fsm-1' },
        application: { operator_ids: [] },
        config: { num_bins: 200, start: 0, end: 1 },
      },
    };
    store.set(visibleEntriesAtom, { 'resource-1': request });
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );

    const { result } = renderHook(() => useReturnedTimelineNumBins('resource-1'), { wrapper });

    expect(result.current).toBeUndefined();
  });

  it('returns undefined while the cached response belongs to the previous viewport', () => {
    const store = createStore();
    const request: TimelineRequest<OperatorFilter> = {
      Resource: {
        resource_id: 'resource-1',
        long_entities_threshold_s: null,
        entity_filter: { entity_type_name: 'fsm-1' },
        application: { operator_ids: [] },
        config: { num_bins: 200, start: 0.25, end: 1 },
      },
    };
    const cacheKey = timelineCacheKey({
      resourceId: 'resource-1',
      resourceTypeName: '',
      fsmTypeName: 'fsm-1',
    });
    const response: SingleTimelineResponse = {
      config: {
        span: { start: 0, end: 1 },
        bin_duration: 0.0025,
        num_bins: 400n,
      },
      data: {} as SingleTimelineResponse['data'],
    };
    store.set(visibleEntriesAtom, { 'resource-1': request });
    store.set(timelineDataMapAtom, { [cacheKey]: response });
    store.set(debouncedZoomRangeAtom, { start: 0.25, end: 1 });
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );

    const { result } = renderHook(
      () => ({
        numBins: useReturnedTimelineNumBins('resource-1'),
        isStale: useReturnedTimelineIsStale('resource-1'),
      }),
      { wrapper }
    );

    expect(result.current).toEqual({ numBins: undefined, isStale: true });
  });
});

describe('useReturnedTimelineActivity', () => {
  it('marks an all-zero binned timeline as inactive', () => {
    const store = createStore();
    store.set(visibleEntriesAtom, { 'resource-1': makeTimelineRequest() });
    store.set(timelineDataMapAtom, {
      [timelineCacheKeyForTest()]: makeTimelineResponse({
        Binned: {
          config: { span: { start: 0, end: 1 }, bin_duration: 0.5, num_bins: 2n },
          capacities_values: { unit: [0, 0], memory: [0, 0] },
          long_fsms: [],
        },
      }),
    });
    store.set(debouncedZoomRangeAtom, { start: 0, end: 1 });
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );

    const { result } = renderHook(() => useReturnedTimelineActivity(), { wrapper });

    expect(result.current.get('resource-1')).toBe(false);
  });

  it('marks binned-by-state data with a nonzero bin as active', () => {
    const store = createStore();
    store.set(visibleEntriesAtom, { 'resource-1': makeTimelineRequest() });
    store.set(timelineDataMapAtom, {
      [timelineCacheKeyForTest()]: makeTimelineResponse({
        BinnedByState: {
          config: { span: { start: 0, end: 1 }, bin_duration: 0.5, num_bins: 2n },
          capacities_states_values: {
            unit: { queued: [0, 0], running: [0, 1] },
          },
          long_fsms: [],
        },
      }),
    });
    store.set(debouncedZoomRangeAtom, { start: 0, end: 1 });
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );

    const { result } = renderHook(() => useReturnedTimelineActivity(), { wrapper });

    expect(result.current.get('resource-1')).toBe(true);
  });

  it('omits activity from a cached response for a previous viewport', () => {
    const store = createStore();
    store.set(visibleEntriesAtom, { 'resource-1': makeTimelineRequest() });
    store.set(timelineDataMapAtom, {
      [timelineCacheKeyForTest()]: makeTimelineResponse(
        {
          Binned: {
            config: { span: { start: 0, end: 1 }, bin_duration: 0.5, num_bins: 2n },
            capacities_values: { unit: [0, 0] },
            long_fsms: [],
          },
        },
        { start: 0, end: 1 }
      ),
    });
    store.set(debouncedZoomRangeAtom, { start: 2, end: 3 });
    const wrapper = ({ children }: PropsWithChildren) => (
      <Provider store={store}>{children}</Provider>
    );

    const { result } = renderHook(() => useReturnedTimelineActivity(), { wrapper });

    expect(result.current.has('resource-1')).toBe(false);
  });
});
