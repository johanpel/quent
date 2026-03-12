import { atom } from 'jotai';
import type { StatValue } from '@/services/query-plan/types';

export interface HoveredOperatorInfo {
  nodeId: string;
  label: string;
  operationType: string;
  stats: Array<{ key: string; value: StatValue }>;
}

/** The set of currently selected node IDs in the DAG chart */
export const selectedNodeIdsAtom = atom(new Set<string>());

/** Display label of the currently selected operator (set alongside selectedNodeIdsAtom) */
export const selectedOperatorLabelAtom = atom<string | null>(null);

/** The currently selected plan ID in the query plan tree view */
export const selectedPlanIdAtom = atom<string>('');

/** Worker ID of the query plan tree item currently being hovered */
export const hoveredWorkerIdAtom = atom<string | null>(null);

/** Operator ID currently being hovered (shared between DAG and table) */
export const hoveredOperatorIdAtom = atom<string | null>(null);

/** Full info for the operator being hovered in the DAG (drives the stats overlay) */
export const hoveredOperatorInfoAtom = atom<HoveredOperatorInfo | null>(null);
