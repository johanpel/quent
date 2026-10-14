// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useMatch, useNavigate } from '@tanstack/react-router';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ChevronDown, ChevronRight, Home } from 'lucide-react';
import {
  DataText,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
  OverflowHoverCardContent,
  OverflowingItemLabel,
  useOverflowHoverCard,
} from '@quent/components';
import { cn, formatAttributeValue } from '@quent/utils';
import type { DynamicAttribute } from '@quent/utils';
import { fetchListEngines, queriesQueryOptions, queryBundleQueryOptions } from '@quent/client';
import { attributeLabel, sortQueriesByMostRecent } from '@/components/query-selection/catalog';

interface BreadcrumbItem {
  id: string;
  label: string;
  details?: BreadcrumbItemDetail[];
}

interface BreadcrumbItemDetail {
  label: string;
  value: string;
}

function attributeDetails(attributes: DynamicAttribute[]): BreadcrumbItemDetail[] {
  return attributes.map(attribute => ({
    label: attributeLabel(attribute.key),
    value: formatAttributeValue(attribute.key, attribute.value),
  }));
}

function BreadcrumbDropdownItem({
  item,
  active,
  open,
  onOpenChange,
  onSelect,
}: {
  item: BreadcrumbItem;
  active: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: () => void;
}) {
  const menuItem = (
    <DropdownMenuItem
      onSelect={onSelect}
      className={cn('min-w-48', active && 'bg-accent font-semibold')}
    >
      <OverflowingItemLabel label={item.label} />
    </DropdownMenuItem>
  );

  if (!item.details?.length) {
    return menuItem;
  }

  return (
    <HoverCard open={open} onOpenChange={onOpenChange} openDelay={100} closeDelay={50}>
      <HoverCardTrigger asChild>{menuItem}</HoverCardTrigger>
      <HoverCardContent side="right" align="start" className="w-80 p-3 duration-75">
        <dl className="space-y-2">
          {item.details.map((detail, index) => (
            <div
              key={`${detail.label}-${index}`}
              className="grid grid-cols-[max-content_minmax(0,1fr)] gap-4"
            >
              <dt className="text-xs text-muted-foreground">{detail.label}</dt>
              <dd className="min-w-0 text-right text-xs">
                <DataText className="whitespace-normal break-words">{detail.value}</DataText>
              </dd>
            </div>
          ))}
        </dl>
      </HoverCardContent>
    </HoverCard>
  );
}

function BreadcrumbDropdown({
  label,
  activeId,
  items,
  loading,
  error,
  onSelect,
}: {
  label: string;
  activeId: string;
  items: BreadcrumbItem[] | undefined;
  loading: boolean;
  error: boolean;
  onSelect: (id: string) => void;
}) {
  const labelRef = useRef<HTMLSpanElement>(null);
  const [openItemId, setOpenItemId] = useState<string | null>(null);
  const {
    open: labelHoverOpen,
    handlePointerEnter,
    handlePointerLeave,
  } = useOverflowHoverCard(labelRef);

  return (
    <HoverCard open={labelHoverOpen}>
      <DropdownMenu onOpenChange={open => !open && setOpenItemId(null)}>
        <HoverCardTrigger asChild>
          <DropdownMenuTrigger asChild>
            <button
              aria-label={`Change ${label}`}
              className="-mx-1.5 flex min-w-0 max-w-40 cursor-pointer items-center gap-0.5 rounded-sm px-1.5 py-0.5 transition-colors hover:bg-accent hover:text-foreground md:max-w-48 xl:max-w-64"
              onPointerEnter={handlePointerEnter}
              onPointerLeave={handlePointerLeave}
              onBlur={handlePointerLeave}
            >
              <span ref={labelRef} className="min-w-0 truncate">
                <DataText>{label}</DataText>
              </span>
              <ChevronDown className="h-3 w-3 shrink-0 opacity-50" />
            </button>
          </DropdownMenuTrigger>
        </HoverCardTrigger>
        <DropdownMenuContent align="start" className="max-h-64 w-max max-w-64 overflow-y-auto">
          {items?.map(item => (
            <BreadcrumbDropdownItem
              key={item.id}
              item={item}
              active={item.id === activeId}
              open={openItemId === item.id}
              onOpenChange={open =>
                setOpenItemId(current => (open ? item.id : current === item.id ? null : current))
              }
              onSelect={() => onSelect(item.id)}
            />
          ))}
          {loading && <DropdownMenuItem disabled>Loading…</DropdownMenuItem>}
          {error && <DropdownMenuItem disabled>Unable to load items</DropdownMenuItem>}
          {!loading && !error && (!items || items.length === 0) && (
            <DropdownMenuItem disabled>No items</DropdownMenuItem>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
      <OverflowHoverCardContent label={label} side="bottom" />
    </HoverCard>
  );
}

export function NavBarNavigator() {
  const queryLayoutMatch = useMatch({
    from: '/profile/engine/$engineId/query/$queryId',
    shouldThrow: false,
  });
  const engineId = queryLayoutMatch?.params?.engineId;
  const queryId = queryLayoutMatch?.params?.queryId;

  if (!engineId) {
    return null;
  }

  return (
    <NavBarNavigatorContent
      key={`${engineId}:${queryId ?? ''}`}
      engineId={engineId}
      queryId={queryId}
    />
  );
}

function NavBarNavigatorContent({
  engineId,
  queryId,
}: {
  engineId: string;
  queryId: string | undefined;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [navigationError, setNavigationError] = useState('');
  const switchToken = useRef(0);
  useEffect(
    () => () => {
      switchToken.current += 1;
    },
    []
  );
  const { data: queryBundle } = useQuery({
    ...queryBundleQueryOptions({ engineId: engineId ?? '', queryId: queryId ?? '' }),
    enabled: !!engineId && !!queryId,
  });
  const enginesQuery = useQuery({
    queryKey: ['list_engines'],
    queryFn: fetchListEngines,
    enabled: !!engineId,
  });
  const queriesQuery = useQuery(queriesQueryOptions(engineId ?? ''));
  const sortedQueries = useMemo(
    () => sortQueriesByMostRecent(queriesQuery.data?.items ?? []),
    [queriesQuery.data?.items]
  );

  if (!queryBundle) {
    return null;
  }

  const engineItems =
    enginesQuery.data?.items.map(engine => ({
      id: engine.id,
      label: engine.instance_name ?? engine.id,
      details: attributeDetails(engine.custom_attributes),
    })) ?? [];
  const queryItems = sortedQueries.map(query => ({
    id: query.id,
    label: query.instance_name ?? query.id,
    details: attributeDetails(query.custom_attributes),
  }));
  const engineLabel = queryBundle.entities.engine.instance_name ?? queryBundle.entities.engine.id;
  const queryLabel = queryBundle.entities.query.instance_name ?? queryBundle.entities.query.id;

  const handleEngineChange = async (newEngineId: string) => {
    if (newEngineId === engineId) {
      return;
    }
    const token = ++switchToken.current;
    setNavigationError('');
    try {
      const response = await queryClient.fetchQuery(queriesQueryOptions(newEngineId));
      if (token !== switchToken.current) {
        return;
      }
      const firstQuery = sortQueriesByMostRecent(response.items)[0];
      if (!firstQuery) {
        setNavigationError('That engine has no queries.');
        return;
      }
      navigate({
        to: '/profile/engine/$engineId/query/$queryId',
        params: { engineId: newEngineId, queryId: firstQuery.id },
        search: {},
      });
    } catch (error) {
      if (token !== switchToken.current) {
        return;
      }
      setNavigationError(error instanceof Error ? error.message : 'Unable to switch engines.');
    }
  };

  const handleQueryChange = (newQueryId: string) => {
    switchToken.current += 1;
    if (newQueryId === queryId) {
      return;
    }
    setNavigationError('');
    navigate({
      to: '/profile/engine/$engineId/query/$queryId',
      params: { engineId, queryId: newQueryId },
      search: {},
    });
  };

  return (
    <div className="flex min-w-0 items-center gap-2">
      <nav
        aria-label="Profile selection"
        className="flex min-w-0 max-w-full items-center gap-1.5 text-sm text-muted-foreground"
      >
        <Link
          to="/"
          aria-label="Select an engine"
          title="Select an engine"
          className="flex size-7 shrink-0 items-center justify-center rounded-sm transition-colors hover:bg-accent hover:text-foreground"
        >
          <Home className="size-4" />
        </Link>
        <ChevronRight className="h-3.5 w-3.5 shrink-0" />
        <BreadcrumbDropdown
          label={engineLabel}
          activeId={engineId}
          items={engineItems}
          loading={enginesQuery.isLoading}
          error={enginesQuery.isError}
          onSelect={handleEngineChange}
        />
        <ChevronRight className="h-3.5 w-3.5 shrink-0" />
        <BreadcrumbDropdown
          label={queryLabel}
          activeId={queryId ?? ''}
          items={queryItems}
          loading={queriesQuery.isLoading}
          error={queriesQuery.isError}
          onSelect={handleQueryChange}
        />
      </nav>
      {navigationError && (
        <span role="alert" className="max-w-48 truncate text-xs text-destructive">
          {navigationError}
        </span>
      )}
    </div>
  );
}
