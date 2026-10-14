// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { http, HttpResponse } from 'msw';

export const handlers = [
  http.get('/api/engines', () => {
    return HttpResponse.json({
      items: [
        {
          id: 'engine-1',
          instance_name: 'Engine 1',
          start_time_unix_ns: null,
          duration_s: null,
          custom_attributes: [
            { key: 'num_workloads', value: { U64: 1 } },
            { key: 'num_queries', value: { U64: 1 } },
          ],
          implementation: null,
        },
      ],
      initial_group_by_attribute: null,
    });
  }),

  http.get('/api/engines/:engineId/queries', () => {
    return HttpResponse.json({
      items: [
        {
          id: 'query-1',
          instance_name: 'Query 1',
          custom_attributes: [],
          start_unix_ns: null,
          planning_s: null,
          executing_s: null,
          completed_s: null,
        },
      ],
      initial_group_by_attribute: null,
    });
  }),

  // Example: Get query details
  http.get('/api/queries/:queryId', ({ params }) => {
    const { queryId } = params;
    return HttpResponse.json({
      id: queryId,
      status: 'completed',
      createdAt: new Date().toISOString(),
    });
  }),

  // Example: Get node profile data
  http.get('/api/queries/:queryId/nodes/:nodeId/profile', ({ params }) => {
    const { nodeId } = params;
    const timestamps = Array.from({ length: 100 }, (_, i) => Date.now() - i * 1000);
    return HttpResponse.json({
      nodeId,
      timestamps,
      series: {
        CPU: Array.from({ length: 100 }, () => Math.random() * 100),
        Memory: Array.from({ length: 100 }, () => Math.random() * 1000),
        IO: Array.from({ length: 100 }, () => Math.random() * 500),
      },
    });
  }),
];
