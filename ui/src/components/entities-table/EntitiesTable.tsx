// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState } from 'react';
import {
  Check,
  ChevronFirst,
  ChevronLast,
  ChevronsUpDown,
  LoaderCircle,
  RotateCcw,
} from 'lucide-react';
import { useEntities } from '@quent/client';
import { useSelectedNodeIds } from '@quent/hooks';
import {
  Button,
  Input,
  Popover,
  PopoverContent,
  PopoverTrigger,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
  SelectField,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@quent/components';
import type { SelectFieldOption } from '@quent/components';
import { formatAttributeValue, formatDuration } from '@quent/utils';
import type {
  Attribute,
  EntityListRequest,
  FiniteStateMachine,
  QueryBundle,
  QueryFilter,
  OperatorFilter,
  EntityRef,
  SortDir,
} from '@quent/utils';

interface EntitiesTableProps {
  engineId: string;
  queryId: string;
  queryBundle: QueryBundle<EntityRef>;
}

/** Filter state mirroring what `POST /engines/{id}/entities` accepts. */
interface Filters {
  operatorId: string | null;
  entityType: string | null;
  resourceId: string | null;
  minUsageS: string;
  windowStart: string;
  windowEnd: string;
  sortDir: SortDir;
  pageSize: number | null;
}

const SORT_DIR_OPTIONS: SelectFieldOption[] = [
  { value: 'Desc', label: 'Longest resource usage first' },
  { value: 'Asc', label: 'Shortest resource usage first' },
];

const FILTER_DEBOUNCE_MS = 300;
const DEFAULT_PAGE_SIZE = 50;
const MAX_PAGE_SIZE = 500;

/** First and last transition timestamps (seconds relative to the query epoch). */
function fsmSpan(fsm: FiniteStateMachine): { start: number; end: number } {
  let start = Infinity;
  let end = -Infinity;
  for (const t of fsm.transitions) {
    if (t.timestamp < start) start = t.timestamp;
    if (t.timestamp > end) end = t.timestamp;
  }
  return fsm.transitions.length === 0 ? { start: 0, end: 0 } : { start, end };
}

/** Parse a numeric text field, returning null when empty or invalid. */
function parseOptionalNumber(value: string): number | null {
  if (value.trim() === '') return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizePageSize(value: number | null): number {
  if (value === null || !Number.isFinite(value)) return DEFAULT_PAGE_SIZE;
  return Math.min(MAX_PAGE_SIZE, Math.max(1, Math.trunc(value)));
}

function defaultFilters(durationS: number): Filters {
  return {
    operatorId: null,
    entityType: null,
    resourceId: null,
    minUsageS: '',
    windowStart: '0',
    windowEnd: String(durationS),
    sortDir: 'Desc',
    pageSize: DEFAULT_PAGE_SIZE,
  };
}

/** Value that only updates after `ms` of stability, keeping fast inputs off the fetch key. */
function useDebounced<T>(value: T, ms: number): T {
  const [debounced, setDebounced] = useState(value);
  // Key on the serialization: `value` is a fresh object each render, so a
  // referential dep would reset the timer every render.
  const key = JSON.stringify(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), ms);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, ms]);
  return debounced;
}

interface NumberFieldProps {
  label: string;
  value: string;
  width: string;
  min?: number;
  onChange: (value: string) => void;
}

function NumberField({ label, value, width, min, onChange }: NumberFieldProps) {
  return (
    <label className="flex flex-col gap-1 text-xs text-muted-foreground">
      {label}
      <Input
        type="number"
        min={min}
        step="any"
        className={`h-8 ${width}`}
        value={value}
        onChange={e => onChange(e.target.value)}
      />
    </label>
  );
}

interface PageSizeFieldProps {
  value: number | null;
  onChange: (value: number | null) => void;
}

function PageSizeField({ value, onChange }: PageSizeFieldProps) {
  return (
    <label className="flex flex-col gap-1 text-xs text-muted-foreground">
      Page size
      <Input
        type="number"
        min={1}
        max={MAX_PAGE_SIZE}
        step={1}
        className="h-8 w-24"
        value={value ?? ''}
        onChange={e => onChange(e.target.value === '' ? null : e.target.valueAsNumber)}
        onBlur={() => onChange(normalizePageSize(value))}
      />
    </label>
  );
}

interface SearchableSelectProps {
  label: string;
  value: string | null;
  options: SelectFieldOption[];
  placeholder: string;
  onValueChange: (value: string | null) => void;
}

function SearchableSelect({
  label,
  value,
  options,
  placeholder,
  onValueChange,
}: SearchableSelectProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const selected = options.find(option => option.value === value);
  const normalizedSearch = search.trim().toLocaleLowerCase();
  const filteredOptions = options.filter(option =>
    `${option.label ?? option.value} ${option.value}`.toLocaleLowerCase().includes(normalizedSearch)
  );

  const select = (nextValue: string | null) => {
    onValueChange(nextValue);
    setOpen(false);
    setSearch('');
  };

  return (
    <label className="flex items-center gap-1.5 text-xs text-muted-foreground">
      <span className="shrink-0">{label}</span>
      <Popover
        open={open}
        onOpenChange={nextOpen => {
          setOpen(nextOpen);
          if (!nextOpen) setSearch('');
        }}
      >
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            role="combobox"
            aria-expanded={open}
            className="h-8 w-56 justify-between px-2 font-normal"
          >
            <span className="truncate text-xs">
              {selected?.label ?? selected?.value ?? placeholder}
            </span>
            <ChevronsUpDown className="ml-2 h-3.5 w-3.5 shrink-0 opacity-50" />
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-64 p-2" align="start">
          <Input
            autoFocus
            value={search}
            onChange={event => setSearch(event.target.value)}
            placeholder={`Search ${label.toLocaleLowerCase()}…`}
            aria-label={`Search ${label.toLocaleLowerCase()}`}
            className="h-8"
          />
          <div className="mt-2 max-h-56 overflow-auto" role="listbox" aria-label={label}>
            <button
              type="button"
              role="option"
              aria-selected={value === null}
              className="flex w-full items-center rounded px-2 py-1.5 text-left text-xs hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => select(null)}
            >
              <Check className={`mr-2 h-3.5 w-3.5 ${value === null ? '' : 'opacity-0'}`} />
              {placeholder}
            </button>
            {filteredOptions.map(option => (
              <button
                key={option.value}
                type="button"
                role="option"
                aria-selected={option.value === value}
                className="flex w-full items-center rounded px-2 py-1.5 text-left text-xs hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onClick={() => select(option.value)}
              >
                <Check
                  className={`mr-2 h-3.5 w-3.5 shrink-0 ${option.value === value ? '' : 'opacity-0'}`}
                />
                <span className="truncate">{option.label ?? option.value}</span>
              </button>
            ))}
            {filteredOptions.length === 0 && (
              <div className="px-2 py-3 text-center text-xs text-muted-foreground">No matches.</div>
            )}
          </div>
        </PopoverContent>
      </Popover>
    </label>
  );
}

/** Key/value rows for a state's recorded or derived attributes. */
function AttributeRows({ attrs, derived }: { attrs: Attribute[]; derived?: boolean }) {
  return (
    <ul className={`mt-1 space-y-0.5 text-xs ${derived ? 'italic text-muted-foreground' : ''}`}>
      {attrs.map((a, k) => (
        <li key={k} className="flex justify-between gap-3">
          <span className={derived ? '' : 'text-muted-foreground'}>{a.key}</span>
          <span className="tabular-nums">{formatAttributeValue(a.key, a.value)}</span>
        </li>
      ))}
    </ul>
  );
}

interface EntityDetailPanelProps {
  fsm: FiniteStateMachine | null;
  /** Resolve a resource ID to a human-readable label. */
  resourceLabel: (id: string) => string;
}

/** Right-hand detail panel listing the state sequence of the selected FSM. */
function EntityDetailPanel({ fsm, resourceLabel }: EntityDetailPanelProps) {
  if (!fsm) {
    return (
      <div className="flex h-full items-center justify-center p-4 text-center text-sm text-muted-foreground">
        Select an entity to view its states.
      </div>
    );
  }
  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b bg-card p-3">
        <div className="text-sm font-medium">{fsm.instance_name}</div>
        <div className="text-xs text-muted-foreground">{fsm.type_name}</div>
        <div className="mt-1 break-all font-mono text-xs text-muted-foreground">{fsm.id}</div>
      </div>
      <ol className="min-h-0 flex-1 space-y-2 overflow-auto p-3">
        {fsm.transitions.map((t, i) => (
          <li key={i} className="rounded border bg-card p-2">
            <div className="flex items-baseline justify-between gap-2">
              <span className="text-sm font-medium">
                <span className="text-muted-foreground">{i + 1}.</span> {t.name}
              </span>
              <span className="tabular-nums text-xs text-muted-foreground">
                {t.timestamp.toFixed(3)}s
                {fsm.transitions[i + 1] && (
                  <>
                    {' '}
                    · for {formatDuration((fsm.transitions[i + 1].timestamp - t.timestamp) * 1000)}
                  </>
                )}
              </span>
            </div>
            {t.usages.length > 0 && (
              <ul className="mt-1 space-y-0.5 text-xs text-muted-foreground">
                {t.usages.map((u, j) => (
                  <li key={j} className="flex flex-wrap items-baseline gap-x-2">
                    <span className="font-mono">{resourceLabel(u.resource)}</span>
                    {u.capacities.map(([name, capacity], k) => (
                      <span key={k} className="tabular-nums">
                        {name}
                        {capacity != null ? `=${capacity}` : ''}
                      </span>
                    ))}
                  </li>
                ))}
              </ul>
            )}
            {t.attributes.length > 0 && <AttributeRows attrs={t.attributes} />}
            {t.derived_attributes.length > 0 && (
              <AttributeRows attrs={t.derived_attributes} derived />
            )}
          </li>
        ))}
      </ol>
    </div>
  );
}

export function EntitiesTable({ engineId, queryId, queryBundle }: EntitiesTableProps) {
  const { entities, duration_s } = queryBundle;
  const defaults = defaultFilters(duration_s);
  const selectedNodeIds = useSelectedNodeIds();
  const dagOperatorId = selectedNodeIds.values().next().value ?? null;

  const [filters, setFilters] = useState<Filters>(() => ({
    ...defaultFilters(duration_s),
    operatorId: dagOperatorId,
  }));
  const [page, setPage] = useState(0);
  // The selected FSM shown in the detail panel; kept by value so it survives paging.
  const [selected, setSelected] = useState<FiniteStateMachine | null>(null);

  useEffect(() => {
    setFilters(prev =>
      prev.operatorId === dagOperatorId ? prev : { ...prev, operatorId: dagOperatorId }
    );
    setPage(0);
    setSelected(null);
  }, [dagOperatorId]);

  // Changing any filter invalidates the current page offset.
  const updateFilters = (patch: Partial<Filters>, clearSelection = true) => {
    setFilters(prev => ({ ...prev, ...patch }));
    setPage(0);
    if (clearSelection) setSelected(null);
  };

  const resetFilters = () => {
    setFilters(defaults);
    setPage(0);
    setSelected(null);
  };

  const resourceLabel = (id: string) => {
    const r = entities.resources[id];
    return r ? `${r.instance_name} (${r.type_name})` : id;
  };

  const operatorOptions = useMemo<SelectFieldOption[]>(
    () =>
      Object.values(entities.operators)
        .map(op => ({
          value: op.id,
          label: op.instance_name ?? op.operator_type_name ?? op.id,
        }))
        .sort((a, b) => a.label.localeCompare(b.label)),
    [entities.operators]
  );

  const entityTypeOptions = useMemo<SelectFieldOption[]>(
    () =>
      Object.keys(entities.fsm_types)
        .sort()
        .map(name => ({ value: name })),
    [entities.fsm_types]
  );

  const resourceOptions = useMemo<SelectFieldOption[]>(
    () =>
      Object.values(entities.resources)
        .map(r => ({
          value: r.id,
          label: `${r.instance_name} (${r.type_name})`,
        }))
        .sort((a, b) => a.label.localeCompare(b.label)),
    [entities.resources]
  );

  const pageSize = normalizePageSize(filters.pageSize);
  const windowStart = parseOptionalNumber(filters.windowStart);
  const windowEnd = parseOptionalNumber(filters.windowEnd);
  const minUsageS = parseOptionalNumber(filters.minUsageS);
  const validationErrors: string[] = [];
  if (filters.windowStart.trim() !== '' && windowStart === null)
    validationErrors.push('Window start must be a number.');
  if (filters.windowEnd.trim() !== '' && windowEnd === null)
    validationErrors.push('Window end must be a number.');
  if (filters.minUsageS.trim() !== '' && minUsageS === null)
    validationErrors.push('Minimum usage must be a number.');
  if (windowStart !== null && windowStart < 0)
    validationErrors.push('Window start cannot be negative.');
  if (windowEnd !== null && windowEnd < 0) validationErrors.push('Window end cannot be negative.');
  if (windowStart !== null && windowEnd !== null && windowStart > windowEnd) {
    validationErrors.push('Window start must not exceed window end.');
  }
  if (minUsageS !== null && minUsageS < 0)
    validationErrors.push('Minimum usage cannot be negative.');

  const activeFilterCount = [
    filters.operatorId !== null,
    filters.entityType !== null,
    filters.resourceId !== null,
    filters.minUsageS !== '',
    filters.windowStart !== defaults.windowStart,
    filters.windowEnd !== defaults.windowEnd,
  ].filter(Boolean).length;
  const hasNonDefaultSettings =
    activeFilterCount > 0 ||
    filters.sortDir !== defaults.sortDir ||
    filters.pageSize !== defaults.pageSize;

  const request: EntityListRequest<QueryFilter, OperatorFilter> = {
    entry: {
      window: {
        start: windowStart ?? 0,
        end: windowEnd ?? duration_s,
      },
      filter: {
        scope: filters.resourceId ? { Resource: { resource_id: filters.resourceId } } : null,
        entity_type_name: filters.entityType,
        min_usage_s: minUsageS,
      },
      sort: { key: 'UsageDuration', dir: filters.sortDir },
      page: { max: pageSize, page },
      application: { operator_id: filters.operatorId },
    },
    app_params: { query_id: queryId },
  };

  // Debounce so typing in the numeric inputs does not fire a request per keystroke.
  const debouncedRequest = useDebounced(request, FILTER_DEBOUNCE_MS);
  const { data, isLoading, isFetching, isError, error } = useEntities(
    { engineId, request: debouncedRequest },
    { enabled: validationErrors.length === 0 }
  );
  const requestPending = isFetching || JSON.stringify(request) !== JSON.stringify(debouncedRequest);

  const total = data?.total ?? 0;
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  const rows = useMemo(
    () =>
      (data?.items ?? []).map(item => ({
        fsm: item.entity,
        usageDurationS: item.usage_duration_s,
        ...fsmSpan(item.entity),
      })),
    [data?.items]
  );
  const visibleStart = total === 0 ? 0 : page * pageSize + 1;
  const visibleEnd = total === 0 ? 0 : Math.min(total, visibleStart + rows.length - 1);
  const paginationDisabled = requestPending || validationErrors.length > 0;

  return (
    <ResizablePanelGroup orientation="horizontal" className="h-full">
      <ResizablePanel defaultSize="65%" minSize="40%">
        <div className="flex h-full min-h-0 flex-col">
          <div className="shrink-0 border-b bg-card p-3 flex flex-wrap items-end gap-3">
            <SearchableSelect
              label="Operator"
              placeholder="All operators"
              options={operatorOptions}
              value={filters.operatorId}
              onValueChange={v => updateFilters({ operatorId: v })}
            />
            <SelectField
              label="Type"
              className="w-40"
              placeholder="All types"
              options={entityTypeOptions}
              value={filters.entityType ?? ''}
              onValueChange={v => updateFilters({ entityType: v })}
            />
            <SearchableSelect
              label="Resource"
              placeholder="All resources"
              options={resourceOptions}
              value={filters.resourceId}
              onValueChange={v => updateFilters({ resourceId: v })}
            />
            <NumberField
              label="Min usage (s)"
              width="w-28"
              min={0}
              value={filters.minUsageS}
              onChange={v => updateFilters({ minUsageS: v })}
            />
            <NumberField
              label="Window start (s)"
              width="w-28"
              min={0}
              value={filters.windowStart}
              onChange={v => updateFilters({ windowStart: v })}
            />
            <NumberField
              label="Window end (s)"
              width="w-28"
              min={0}
              value={filters.windowEnd}
              onChange={v => updateFilters({ windowEnd: v })}
            />
            <SelectField
              label="Sort"
              className="w-60"
              clearable={false}
              options={SORT_DIR_OPTIONS}
              value={filters.sortDir}
              onValueChange={v =>
                updateFilters({ sortDir: (v as SortDir | null) ?? 'Desc' }, false)
              }
            />
            <PageSizeField
              value={filters.pageSize}
              onChange={v => updateFilters({ pageSize: v }, false)}
            />
            <Button
              variant="outline"
              size="sm"
              disabled={!hasNonDefaultSettings}
              onClick={resetFilters}
            >
              <RotateCcw className="mr-1.5 h-3.5 w-3.5" />
              Reset filters
            </Button>
            {activeFilterCount > 0 && (
              <span className="pb-1 text-xs text-muted-foreground">
                {activeFilterCount} active {activeFilterCount === 1 ? 'filter' : 'filters'}
              </span>
            )}
            {requestPending && validationErrors.length === 0 && (
              <span
                role="status"
                aria-live="polite"
                className="flex items-center gap-1 pb-1 text-xs text-muted-foreground"
              >
                <LoaderCircle className="h-3.5 w-3.5 animate-spin" />
                Updating…
              </span>
            )}
            {validationErrors.length > 0 && (
              <div role="alert" className="basis-full text-xs text-destructive">
                {validationErrors.join(' ')}
              </div>
            )}
          </div>

          <div
            aria-busy={requestPending}
            className={`flex-1 min-h-0 overflow-auto transition-opacity duration-150 ${
              requestPending && rows.length > 0 ? 'opacity-60' : 'opacity-100'
            }`}
          >
            {isError ? (
              <div className="p-4 text-sm text-destructive">
                Failed to load entities: {error instanceof Error ? error.message : 'unknown error'}
              </div>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Instance</TableHead>
                    <TableHead>Type</TableHead>
                    <TableHead className="text-right">States</TableHead>
                    <TableHead className="text-right">Start</TableHead>
                    <TableHead className="text-right">End</TableHead>
                    <TableHead className="text-right">FSM span</TableHead>
                    <TableHead className="text-right">Longest usage</TableHead>
                    <TableHead>ID</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {rows.map(({ fsm, start, end, usageDurationS }) => (
                    <TableRow
                      key={fsm.id}
                      className="cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
                      tabIndex={0}
                      aria-selected={selected?.id === fsm.id}
                      data-state={selected?.id === fsm.id ? 'selected' : undefined}
                      onClick={() => setSelected(fsm)}
                      onKeyDown={event => {
                        if (event.key === 'Enter' || event.key === ' ') {
                          event.preventDefault();
                          setSelected(fsm);
                        }
                      }}
                    >
                      <TableCell className="font-medium">{fsm.instance_name}</TableCell>
                      <TableCell>{fsm.type_name}</TableCell>
                      <TableCell className="text-right tabular-nums">
                        {fsm.transitions.length}
                      </TableCell>
                      <TableCell className="text-right tabular-nums">{start.toFixed(3)}s</TableCell>
                      <TableCell className="text-right tabular-nums">{end.toFixed(3)}s</TableCell>
                      <TableCell className="text-right tabular-nums">
                        {formatDuration((end - start) * 1000)}
                      </TableCell>
                      <TableCell className="text-right tabular-nums">
                        {formatDuration(usageDurationS * 1000)}
                      </TableCell>
                      <TableCell className="font-mono text-xs text-muted-foreground">
                        {fsm.id}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
            {!isError && !isLoading && validationErrors.length === 0 && rows.length === 0 && (
              <div className="p-4 text-sm text-muted-foreground">
                No entities match the filters.
              </div>
            )}
            {isLoading && <div className="p-4 text-sm text-muted-foreground">Loading…</div>}
          </div>

          <div className="shrink-0 border-t bg-card p-2 flex items-center justify-between text-xs text-muted-foreground">
            <span>
              {visibleStart}–{visibleEnd} of {total} {total === 1 ? 'entity' : 'entities'}
            </span>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                aria-label="First page"
                disabled={paginationDisabled || page <= 0}
                onClick={() => setPage(0)}
              >
                <ChevronFirst className="h-4 w-4" />
              </Button>
              <Button
                variant="outline"
                size="sm"
                disabled={paginationDisabled || page <= 0}
                onClick={() => setPage(p => Math.max(0, p - 1))}
              >
                Previous
              </Button>
              <span>
                Page {page + 1} / {pageCount}
              </span>
              <Button
                variant="outline"
                size="sm"
                disabled={paginationDisabled || page + 1 >= pageCount}
                onClick={() => setPage(p => p + 1)}
              >
                Next
              </Button>
              <Button
                variant="outline"
                size="sm"
                aria-label="Last page"
                disabled={paginationDisabled || page + 1 >= pageCount}
                onClick={() => setPage(pageCount - 1)}
              >
                <ChevronLast className="h-4 w-4" />
              </Button>
            </div>
          </div>
        </div>
      </ResizablePanel>
      <ResizableHandle withHandle />
      <ResizablePanel defaultSize="35%" minSize="20%" collapsible collapsedSize="0%">
        <EntityDetailPanel fsm={selected} resourceLabel={resourceLabel} />
      </ResizablePanel>
    </ResizablePanelGroup>
  );
}
