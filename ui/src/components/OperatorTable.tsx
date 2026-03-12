import { useMemo, useState, useCallback, useEffect, useRef } from 'react';
import { useAtom, useAtomValue, useSetAtom } from 'jotai';
import { selectedPlanIdAtom, selectedNodeIdsAtom, hoveredOperatorIdAtom } from '@/atoms/dag';
import { parseCustomStatistics } from '@/lib/queryBundle.utils';
import type { QueryBundle } from '~quent/types/QueryBundle';
import type { EntityRef } from '~quent/types/EntityRef';
import type { StatValue } from '@/services/query-plan/types';
import { cn } from '@/lib/utils';
import { formatWithPrefix } from '@/services/formatters';
import { operatorTypeColor } from '@/services/colors';

type IndexKey = 'worker_plan' | 'parent_operator_type' | 'parent_operator' | 'operator_type' | 'operator';
type AggMode = 'value' | 'sum' | 'mean';
type SortDir = 'asc' | 'desc';

interface FlatRow {
  workerPlanId: string;
  workerPlanLabel: string;
  planId: string;
  parentOperatorType: string;
  parentOperatorName: string;
  operatorType: string;
  operatorName: string;
  operatorId: string;
  statisticName: string;
  value: StatValue;
}

interface GroupKeyEntry {
  key: IndexKey;
  id: string;
  label: string;
}

interface PivotedRow {
  groupKeys: GroupKeyEntry[];
  rowKey: string;
  values: Map<string, StatValue>;
  aggs: Map<string, { sum: number | null; mean: number | null; count: number; isNumeric: boolean }>;
  operatorIds: Set<string>;
  operatorType: string;
  /** Map from operator ID to the plan ID it belongs to */
  operatorPlanIds: Map<string, string>;
}

// --- formatting ---

function formatNumber(n: number | null): string {
  if (n === null) return '-';
  if (Number.isInteger(n)) return n.toLocaleString();
  return n.toLocaleString(undefined, { maximumFractionDigits: 4 });
}

function formatBytes(n: number | null): string {
  if (n === null) return '-';
  return formatWithPrefix(n, 'B', 'Iec', 2);
}

function isBytesStat(name: string): boolean {
  return name.includes('_bytes') || name.endsWith('_byte') || name.startsWith('bytes_');
}

function formatStatValue(value: StatValue, statName: string): string {
  if (value === null || value === undefined) return '-';
  if (typeof value === 'number') {
    return isBytesStat(statName) ? formatBytes(value) : formatNumber(value);
  }
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (Array.isArray(value)) return value.join(', ');
  return String(value);
}

function formatStatNumber(n: number | null, statName: string): string {
  if (n === null) return '-';
  return isBytesStat(statName) ? formatBytes(n) : formatNumber(n);
}

function isNumericValue(v: StatValue): v is number {
  return typeof v === 'number';
}

// --- color gradient ---

/** Interpolate between two RGB colors. t in [0, 1]. */
const GRADIENT_COLOR: [number, number, number] = [239, 68, 68]; // red-500

function gradientBg(value: number, min: number, max: number): string | undefined {
  if (min === max) return undefined;
  const t = (value - min) / (max - min);
  const alpha = t * 0.45; // transparent at low, ~0.45 at high
  return `rgba(${GRADIENT_COLOR[0]}, ${GRADIENT_COLOR[1]}, ${GRADIENT_COLOR[2]}, ${alpha.toFixed(3)})`;
}

// --- grouping helpers ---

function rowGroupKey(row: FlatRow, enabledIndices: IndexKey[]): string {
  return enabledIndices.map(idx => {
    switch (idx) {
      case 'worker_plan': return row.workerPlanId;
      case 'parent_operator_type': return row.parentOperatorType;
      case 'parent_operator': return row.parentOperatorName;
      case 'operator_type': return row.operatorType;
      case 'operator': return row.operatorName;
    }
  }).join('\0');
}

function getGroupKeys(row: FlatRow, enabledIndices: IndexKey[]): GroupKeyEntry[] {
  return enabledIndices.map(idx => {
    switch (idx) {
      case 'worker_plan': return { key: idx, id: row.workerPlanId, label: row.workerPlanLabel };
      case 'parent_operator_type': return { key: idx, id: row.parentOperatorType, label: row.parentOperatorType };
      case 'parent_operator': return { key: idx, id: row.parentOperatorName, label: row.parentOperatorName };
      case 'operator_type': return { key: idx, id: row.operatorType, label: row.operatorType };
      case 'operator': return { key: idx, id: row.operatorName, label: row.operatorName };
    }
  });
}

function computeRowSpans(rows: PivotedRow[]): (number | null)[][] {
  const numCols = rows[0]?.groupKeys.length ?? 0;
  const spans: (number | null)[][] = rows.map(() => new Array(numCols).fill(null));
  if (rows.length === 0) return spans;

  for (let col = 0; col < numCols; col++) {
    let start = 0;
    for (let i = 1; i <= rows.length; i++) {
      const changed = i === rows.length || rows[i].groupKeys.slice(0, col + 1).some(
        (gk, j) => gk.id !== rows[i - 1].groupKeys[j]?.id
      );
      const parentChanged = col > 0 && i < rows.length && rows[i].groupKeys.slice(0, col).some(
        (gk, j) => gk.id !== rows[start].groupKeys[j]?.id
      );
      if (changed || parentChanged) {
        spans[start][col] = i - start;
        start = i;
      }
    }
  }
  return spans;
}

/** Extract the numeric sort value for a stat from a pivoted row. */
function getSortValue(row: PivotedRow, stat: string, isAgg: boolean, aggMode: AggMode): number | null {
  if (!isAgg) {
    const v = row.values.get(stat);
    if (v === undefined) return null;
    return isNumericValue(v) ? v : null;
  }
  const agg = row.aggs.get(stat);
  if (!agg || !agg.isNumeric) return null;
  return aggMode === 'sum' ? agg.sum : agg.mean;
}

// --- component ---

interface OperatorTableProps {
  queryBundle: QueryBundle<EntityRef>;
}

export function OperatorTable({ queryBundle }: OperatorTableProps) {
  const [selectedPlanId, setSelectedPlanId] = useAtom(selectedPlanIdAtom);
  const selectedNodeIds = useAtomValue(selectedNodeIdsAtom);
  const hoveredOperatorId = useAtomValue(hoveredOperatorIdAtom);
  const setHoveredOperatorId = useSetAtom(hoveredOperatorIdAtom);
  const { entities } = queryBundle;
  const rowRefs = useRef<Map<string, HTMLTableRowElement>>(new Map());

  const [indexOrder, setIndexOrder] = useState<IndexKey[]>(['worker_plan', 'parent_operator_type', 'parent_operator', 'operator_type', 'operator']);
  const [enabledIndices, setEnabledIndices] = useState<Record<IndexKey, boolean>>({
    worker_plan: true,
    parent_operator_type: false,
    parent_operator: false,
    operator_type: true,
    operator: true,
  });
  const [draggedIndex, setDraggedIndex] = useState<IndexKey | null>(null);
  const [selectedStats, setSelectedStats] = useState<Set<string> | null>(null);
  const defaultStatsInitialized = useRef(false);
  const [statOrder, setStatOrder] = useState<string[] | null>(null);
  const [aggMode, setAggMode] = useState<AggMode>('sum');

  const [draggedStat, setDraggedStat] = useState<string | null>(null);
  const [sortColumn, setSortColumn] = useState<string | null>(null);
  const [sortDir, setSortDir] = useState<SortDir>('desc');

  const toggleIndex = useCallback((key: IndexKey) => {
    setEnabledIndices(prev => ({ ...prev, [key]: !prev[key] }));
  }, []);

  const handleDragStart = useCallback((key: IndexKey) => {
    setDraggedIndex(key);
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent, targetKey: IndexKey) => {
    e.preventDefault();
    if (!draggedIndex || draggedIndex === targetKey) return;
    setIndexOrder(prev => {
      const next = [...prev];
      const fromIdx = next.indexOf(draggedIndex);
      const toIdx = next.indexOf(targetKey);
      next.splice(fromIdx, 1);
      next.splice(toIdx, 0, draggedIndex);
      return next;
    });
  }, [draggedIndex]);

  const handleDragEnd = useCallback(() => {
    setDraggedIndex(null);
  }, []);

  const handleSort = useCallback((stat: string) => {
    setSortColumn(prev => {
      if (prev === stat) {
        // Toggle direction, or clear on third click
        setSortDir(d => {
          if (d === 'desc') return 'asc';
          // asc → clear
          setSortColumn(null);
          return 'desc';
        });
        return stat;
      }
      setSortDir('desc');
      return stat;
    });
  }, []);

  const activeIndices = useMemo(
    () => indexOrder.filter(k => enabledIndices[k]),
    [indexOrder, enabledIndices],
  );
  const isAggregating = activeIndices.length < indexOrder.length;

  const siblingPlanIds = useMemo(() => {
    const selected = selectedPlanId ? entities.plans[selectedPlanId] : undefined;
    if (!selected) return new Set<string>();
    const parentId = selected.parent;
    const ids = new Set<string>();
    for (const p of Object.values(entities.plans)) {
      if (p && p.parent === parentId) ids.add(p.id);
    }
    return ids;
  }, [entities.plans, selectedPlanId]);

  const flatRows = useMemo(() => {
    const rows: FlatRow[] = [];
    const plans = Object.values(entities.plans)
      .filter((p): p is NonNullable<typeof p> => p != null && siblingPlanIds.has(p.id))
      .sort((a, b) => {
        const wA = a.worker_id ?? '';
        const wB = b.worker_id ?? '';
        if (wA !== wB) return wA.localeCompare(wB);
        return a.id.localeCompare(b.id);
      });

    for (const plan of plans) {
      const worker = plan.worker_id ? entities.workers[plan.worker_id] : undefined;
      const workerPart = worker?.instance_name ?? plan.worker_id ?? '-';
      const planPart = plan.instance_name ?? plan.id;
      const workerPlanLabel = `${workerPart} / ${planPart}`;
      const workerPlanId = `${plan.worker_id ?? '-'}:${plan.id}`;

      const ops = Object.values(entities.operators)
        .filter((op): op is NonNullable<typeof op> => op != null && op.plan_id === plan.id)
        .sort((a, b) => {
          const typeA = a.operator_type_name ?? '';
          const typeB = b.operator_type_name ?? '';
          if (typeA !== typeB) return typeA.localeCompare(typeB);
          const nameA = a.instance_name ?? a.id;
          const nameB = b.instance_name ?? b.id;
          return nameA.localeCompare(nameB);
        });

      for (const op of ops) {
        const operatorName = op.instance_name ?? op.id;
        const operatorType = op.operator_type_name ?? '-';
        const parentOpId = op.parent_operator_ids?.[0];
        const parentOp = parentOpId ? entities.operators[parentOpId] : undefined;
        const parentOperatorType = parentOp?.operator_type_name ?? '-';
        const parentOperatorName = parentOp?.instance_name ?? parentOpId ?? '-';
        const base = { workerPlanId, workerPlanLabel, planId: plan.id, parentOperatorType, parentOperatorName, operatorType, operatorName, operatorId: op.id };

        const duration = op.active_span ? op.active_span.end - op.active_span.start : null;
        rows.push({ ...base, statisticName: 'duration_s', value: duration !== null ? Number(duration.toFixed(6)) : null });

        for (const stat of parseCustomStatistics(op)) {
          rows.push({ ...base, statisticName: stat.key, value: stat.value });
        }
      }
    }
    return rows;
  }, [entities, siblingPlanIds]);

  const allStatNames = useMemo(() => {
    const seen = new Set<string>();
    const names: string[] = [];
    for (const row of flatRows) {
      if (!seen.has(row.statisticName)) {
        seen.add(row.statisticName);
        names.push(row.statisticName);
      }
    }
    return names;
  }, [flatRows]);

  // Initialize default column selection: duration_s, input_*, output_*
  useEffect(() => {
    if (defaultStatsInitialized.current || allStatNames.length === 0) return;
    defaultStatsInitialized.current = true;
    const defaultNames = allStatNames.filter(s => s === 'duration_s' || s.startsWith('input_') || s.startsWith('output_'));
    const defaults = new Set(defaultNames);
    if (defaults.size > 0) {
      setSelectedStats(defaults);
      // Put default columns first in the ordering
      const rest = allStatNames.filter(s => !defaults.has(s));
      setStatOrder([...defaultNames, ...rest]);
    }
  }, [allStatNames]);

  const orderedStatNames = useMemo(() => {
    if (!statOrder) return allStatNames;
    const allSet = new Set(allStatNames);
    const result = statOrder.filter(s => allSet.has(s));
    for (const s of allStatNames) {
      if (!statOrder.includes(s)) result.push(s);
    }
    return result;
  }, [allStatNames, statOrder]);

  const visibleStats = useMemo(
    () => selectedStats ? orderedStatNames.filter(s => selectedStats.has(s)) : orderedStatNames,
    [orderedStatNames, selectedStats],
  );

  const toggleStat = useCallback((stat: string) => {
    setSelectedStats(prev => {
      const current = prev ?? new Set(allStatNames);
      const next = new Set(current);
      if (next.has(stat)) {
        next.delete(stat);
      } else {
        next.add(stat);
      }
      return next;
    });
  }, [allStatNames]);

  const selectAllStats = useCallback(() => setSelectedStats(null), []);
  const selectNoStats = useCallback(() => setSelectedStats(new Set()), []);

  const handleStatDragStart = useCallback((stat: string) => {
    setDraggedStat(stat);
  }, []);

  const handleStatDragOver = useCallback((e: React.DragEvent, targetStat: string) => {
    e.preventDefault();
    if (!draggedStat || draggedStat === targetStat) return;
    setStatOrder(prev => {
      const current = prev ?? [...allStatNames];
      const next = [...current];
      const fromIdx = next.indexOf(draggedStat);
      const toIdx = next.indexOf(targetStat);
      if (fromIdx === -1 || toIdx === -1) return current;
      next.splice(fromIdx, 1);
      next.splice(toIdx, 0, draggedStat);
      return next;
    });
  }, [draggedStat, allStatNames]);

  const handleStatDragEnd = useCallback(() => {
    setDraggedStat(null);
  }, []);

  // Pivot rows
  const pivotedRows = useMemo((): PivotedRow[] => {
    type Accumulator = {
      keys: GroupKeyEntry[];
      rowKey: string;
      values: Map<string, StatValue>;
      aggBuckets: Map<string, { nums: number[]; count: number }>;
      opIds: Set<string>;
      opPlanIds: Map<string, string>;
      operatorType: string;
    };

    const groups = new Map<string, Accumulator>();

    for (const row of flatRows) {
      const rk = rowGroupKey(row, activeIndices);
      let group = groups.get(rk);
      if (!group) {
        group = {
          keys: getGroupKeys(row, activeIndices),
          rowKey: rk,
          values: new Map(),
          aggBuckets: new Map(),
          opIds: new Set(),
          opPlanIds: new Map(),
          operatorType: row.operatorType,
        };
        groups.set(rk, group);
      }
      group.opIds.add(row.operatorId);
      group.opPlanIds.set(row.operatorId, row.planId);

      if (!isAggregating) {
        group.values.set(row.statisticName, row.value);
      } else {
        let bucket = group.aggBuckets.get(row.statisticName);
        if (!bucket) {
          bucket = { nums: [], count: 0 };
          group.aggBuckets.set(row.statisticName, bucket);
        }
        bucket.count++;
        if (isNumericValue(row.value)) {
          bucket.nums.push(row.value);
        }
      }
    }

    const result: PivotedRow[] = [];
    for (const group of groups.values()) {
      const aggs = new Map<string, { sum: number | null; mean: number | null; count: number; isNumeric: boolean }>();
      if (isAggregating) {
        for (const [stat, bucket] of group.aggBuckets) {
          const hasNum = bucket.nums.length > 0;
          const sum = hasNum ? bucket.nums.reduce((a, b) => a + b, 0) : null;
          const mean = hasNum ? sum! / bucket.nums.length : null;
          aggs.set(stat, { sum, mean, count: bucket.count, isNumeric: hasNum });
        }
      }
      result.push({
        groupKeys: group.keys,
        rowKey: group.rowKey,
        values: group.values,
        aggs,
        operatorIds: group.opIds,
        operatorPlanIds: group.opPlanIds,
        operatorType: group.operatorType,
      });
    }
    return result;
  }, [flatRows, activeIndices, isAggregating]);

  // Sort pivoted rows
  const sortedRows = useMemo(() => {
    if (!sortColumn) return pivotedRows;
    const sorted = [...pivotedRows];
    const mul = sortDir === 'asc' ? 1 : -1;
    sorted.sort((a, b) => {
      const va = getSortValue(a, sortColumn, isAggregating, aggMode);
      const vb = getSortValue(b, sortColumn, isAggregating, aggMode);
      if (va === null && vb === null) return 0;
      if (va === null) return 1;
      if (vb === null) return -1;
      return (va - vb) * mul;
    });
    return sorted;
  }, [pivotedRows, sortColumn, sortDir, isAggregating, aggMode]);

  const rowSpans = useMemo(() => computeRowSpans(sortedRows), [sortedRows]);

  // Compute per-column min/max for gradient
  const columnRanges = useMemo(() => {
    const ranges = new Map<string, { min: number; max: number }>();
    for (const stat of visibleStats) {
      let min = Infinity;
      let max = -Infinity;
      for (const row of sortedRows) {
        const v = getSortValue(row, stat, isAggregating, aggMode);
        if (v !== null) {
          if (v < min) min = v;
          if (v > max) max = v;
        }
      }
      if (min !== Infinity) {
        ranges.set(stat, { min, max });
      }
    }
    return ranges;
  }, [sortedRows, visibleStats, isAggregating, aggMode]);

  if (!selectedPlanId) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
        Select a plan on the left to view operators
      </div>
    );
  }

  if (flatRows.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
        No operators in the selected plan
      </div>
    );
  }

  // Scroll table row into view when hovering a DAG node
  useEffect(() => {
    if (!hoveredOperatorId) return;
    const row = sortedRows.find(r => r.operatorIds.has(hoveredOperatorId));
    if (!row) return;
    const el = rowRefs.current.get(row.rowKey);
    el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [hoveredOperatorId, sortedRows]);

  const hasSelection = selectedNodeIds.size > 0;
  const indexLabels: Record<IndexKey, string> = { worker_plan: 'Worker / Plan', parent_operator_type: 'Parent Op Type', parent_operator: 'Parent Operator', operator_type: 'Operator Type', operator: 'Operator Instance' };

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar */}
      <div className="shrink-0 flex items-center border-b border-border bg-card">
        {/* Group-by section */}
        <div className="flex items-center gap-2 px-3 py-1.5 border-r border-border">
          <span className="text-xs text-muted-foreground shrink-0">Group by:</span>
          {indexOrder.map(key => (
            <button
              key={key}
              draggable
              onDragStart={() => handleDragStart(key)}
              onDragOver={e => handleDragOver(e, key)}
              onDragEnd={handleDragEnd}
              onClick={() => toggleIndex(key)}
              className={cn(
                'text-xs px-2 py-0.5 rounded border transition-colors cursor-grab active:cursor-grabbing select-none whitespace-nowrap',
                enabledIndices[key]
                  ? 'bg-primary/10 border-primary/40 text-primary'
                  : 'bg-muted/50 border-border text-muted-foreground',
                draggedIndex === key && 'opacity-50',
              )}
            >
              {indexLabels[key]}
            </button>
          ))}
          {isAggregating && (
            <>
              <span className="text-xs text-muted-foreground shrink-0 ml-2">Show:</span>
              {(['sum', 'mean'] as AggMode[]).map(mode => (
                <button
                  key={mode}
                  onClick={() => setAggMode(mode)}
                  className={cn(
                    'text-xs px-2 py-0.5 rounded border transition-colors',
                    aggMode === mode
                      ? 'bg-primary/10 border-primary/40 text-primary'
                      : 'bg-muted/50 border-border text-muted-foreground',
                  )}
                >
                  {mode}
                </button>
              ))}
            </>
          )}
        </div>
        {/* Column selection section */}
        <div
          className="relative flex-1 min-w-0 flex items-center gap-1 px-3 py-1.5 group/cols"
        >
          <span className="text-xs text-muted-foreground shrink-0 mr-1">Columns:</span>
          <button onClick={selectAllStats} className="text-xs text-primary hover:underline shrink-0">All</button>
          <button onClick={selectNoStats} className="text-xs text-primary hover:underline shrink-0">None</button>
          <div className="flex-1 min-w-0 overflow-hidden flex items-center gap-1">
            {[...orderedStatNames].sort((a, b) => {
              const aChecked = selectedStats ? selectedStats.has(a) : true;
              const bChecked = selectedStats ? selectedStats.has(b) : true;
              if (aChecked !== bChecked) return aChecked ? -1 : 1;
              return 0;
            }).map(stat => {
              const checked = selectedStats ? selectedStats.has(stat) : true;
              return (
                <button
                  key={stat}
                  onClick={() => toggleStat(stat)}
                  className={cn(
                    'text-xs px-1.5 py-0 rounded border transition-colors whitespace-nowrap shrink-0',
                    checked
                      ? 'bg-primary/10 border-primary/40 text-primary'
                      : 'bg-muted/50 border-border text-muted-foreground',
                  )}
                >
                  {stat}
                </button>
              );
            })}
          </div>
          <span className="shrink-0 text-xs text-muted-foreground cursor-default">&hellip;&#x25BE;</span>
          {/* Dropdown on hover when items overflow */}
          <div className="absolute left-0 top-full z-20 w-full bg-card border border-border rounded-b shadow-lg p-2 hidden group-hover/cols:flex flex-wrap gap-1">
            {[...orderedStatNames].sort((a, b) => {
              const aChecked = selectedStats ? selectedStats.has(a) : true;
              const bChecked = selectedStats ? selectedStats.has(b) : true;
              if (aChecked !== bChecked) return aChecked ? -1 : 1;
              return 0;
            }).map(stat => {
              const checked = selectedStats ? selectedStats.has(stat) : true;
              return (
                <button
                  key={stat}
                  onClick={() => toggleStat(stat)}
                  className={cn(
                    'text-xs px-1.5 py-0.5 rounded border transition-colors whitespace-nowrap',
                    checked
                      ? 'bg-primary/10 border-primary/40 text-primary'
                      : 'bg-muted/50 border-border text-muted-foreground',
                  )}
                >
                  {stat}
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* Table */}
      <div className="flex-1 min-h-0 overflow-auto">
        <table className="text-sm border-collapse">
          <thead className="sticky top-0 bg-card z-10">
            <tr className="border-b border-border">
              {activeIndices.map(key => (
                <th key={key} className="text-left px-3 py-2 font-medium text-muted-foreground whitespace-nowrap">
                  {indexLabels[key]}
                </th>
              ))}
              {visibleStats.map(stat => (
                <th
                  key={stat}
                  draggable
                  onDragStart={() => handleStatDragStart(stat)}
                  onDragOver={e => handleStatDragOver(e, stat)}
                  onDragEnd={handleStatDragEnd}
                  onClick={() => handleSort(stat)}
                  className={cn(
                    'text-right px-3 py-2 font-medium text-muted-foreground whitespace-nowrap cursor-pointer select-none hover:text-foreground',
                    draggedStat === stat && 'opacity-50',
                    sortColumn === stat && 'text-foreground',
                  )}
                >
                  {stat}
                  {sortColumn === stat && (
                    <span className="ml-1 text-xs">{sortDir === 'asc' ? '▲' : '▼'}</span>
                  )}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sortedRows.map((row, i) => {
              const spans = rowSpans[i];
              const hasOverlap = [...row.operatorIds].some(id => selectedNodeIds.has(id));
              const isHoveredFromDag = hoveredOperatorId !== null && row.operatorIds.has(hoveredOperatorId);
              const isSelected = hasOverlap;
              const isDimmed = hasSelection && !isSelected && !isHoveredFromDag;
              // Pick a representative operator ID for hover (first in set)
              const firstOpId = row.operatorIds.size === 1 ? [...row.operatorIds][0] : null;
              const firstOpPlanId = firstOpId ? row.operatorPlanIds.get(firstOpId) : undefined;

              return (
                <tr
                  key={row.rowKey}
                  ref={el => { if (el) rowRefs.current.set(row.rowKey, el); else rowRefs.current.delete(row.rowKey); }}
                  className={cn(
                    'border-b border-border/50 hover:bg-muted/50 transition-opacity',
                    isSelected && 'bg-muted/70',
                    isHoveredFromDag && 'bg-primary/10',
                  )}
                  style={{ opacity: isDimmed ? 0.3 : 1 }}
                >
                  {row.groupKeys.map((gk, col) =>
                    spans[col] !== null ? (
                      <td
                        key={gk.key}
                        className={cn(
                          'px-3 py-1.5 whitespace-nowrap align-top border-r border-border/30',
                          gk.key === 'operator' && 'font-medium',
                        )}
                        rowSpan={spans[col]!}
                        style={(gk.key === 'operator_type' || gk.key === 'parent_operator_type') ? { borderLeftWidth: 8, borderLeftColor: operatorTypeColor(gk.id), backgroundColor: `color-mix(in srgb, ${operatorTypeColor(gk.id)} 15%, transparent)` } : undefined}
                        onMouseEnter={gk.key === 'operator' && firstOpId ? () => {
                          if (firstOpPlanId && firstOpPlanId !== selectedPlanId) {
                            setSelectedPlanId(firstOpPlanId);
                          }
                          setHoveredOperatorId(firstOpId);
                        } : undefined}
                        onMouseLeave={gk.key === 'operator' && firstOpId ? () => setHoveredOperatorId(prev => prev === firstOpId ? null : prev) : undefined}
                      >
                        {gk.label}
                      </td>
                    ) : null,
                  )}
                  {visibleStats.map(stat => {
                    const numVal = getSortValue(row, stat, isAggregating, aggMode);
                    const range = columnRanges.get(stat);
                    const bg = numVal !== null && range ? gradientBg(numVal, range.min, range.max) : undefined;

                    if (!isAggregating) {
                      const val = row.values.get(stat) ?? null;
                      return (
                        <td
                          key={stat}
                          className="px-3 py-1.5 whitespace-nowrap text-right font-mono"
                          style={bg ? { backgroundColor: bg } : undefined}
                        >
                          {formatStatValue(val, stat)}
                        </td>
                      );
                    }
                    const agg = row.aggs.get(stat);
                    if (!agg || !agg.isNumeric) {
                      return (
                        <td key={stat} className="px-3 py-1.5 whitespace-nowrap text-right font-mono text-muted-foreground">
                          -
                        </td>
                      );
                    }
                    const displayVal = aggMode === 'sum' ? agg.sum : agg.mean;
                    return (
                      <td
                        key={stat}
                        className="px-3 py-1.5 whitespace-nowrap text-right font-mono"
                        style={bg ? { backgroundColor: bg } : undefined}
                      >
                        {formatStatNumber(displayVal, stat)}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
