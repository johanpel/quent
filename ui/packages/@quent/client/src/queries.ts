// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { queryOptions, useQuery } from '@tanstack/react-query';
import { fetchListQueries } from './api';
import { DEFAULT_STALE_TIME } from './constants';

export const queriesQueryOptions = (engineId: string, options?: { staleTime?: number }) =>
  queryOptions({
    queryKey: ['list_queries', engineId],
    queryFn: () => fetchListQueries(engineId),
    staleTime: options?.staleTime ?? DEFAULT_STALE_TIME,
    enabled: !!engineId,
  });

export const useQueries = (engineId: string, options?: { staleTime?: number }) =>
  useQuery(queriesQueryOptions(engineId, options));
