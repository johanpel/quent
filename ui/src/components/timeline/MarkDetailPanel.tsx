import { useEffect, useRef, useCallback } from 'react';
import { useAtom, useAtomValue, useSetAtom } from 'jotai';
import { X } from 'lucide-react';
import {
  selectedMarkAtom,
  trackedEntityAtom,
  engineIdAtom,
  queryIdAtom,
} from '@/atoms/timeline';
import { fetchEntityFsm } from '@/services/api';
import { formatDuration, formatWithPrefix } from '@/services/formatters';
import { getColorByIndex, getColorForKey } from '@/services/colors';
import { cn } from '@/lib/utils';
import type { FsmTypeDecl } from '~quent/types/FsmTypeDecl';
import type { FsmEntityRef } from '~quent/types/FsmEntityRef';

function buildStateColorMap(
  fsmTypes?: { [key in string]?: FsmTypeDecl }
): Map<string, string> {
  const map = new Map<string, string>();
  if (!fsmTypes) return map;
  for (const decl of Object.values(fsmTypes)) {
    if (!decl) continue;
    for (let i = 0; i < decl.states.length; i++) {
      map.set(decl.states[i]!.name, getColorByIndex(i));
    }
  }
  return map;
}

export function MarkDetailPanel({
  fsmTypes,
}: {
  fsmTypes?: { [key in string]?: FsmTypeDecl };
}) {
  const [selectedMark, setSelectedMark] = useAtom(selectedMarkAtom);
  const setTrackedEntity = useSetAtom(trackedEntityAtom);
  const engineId = useAtomValue(engineIdAtom);
  const queryId = useAtomValue(queryIdAtom);
  const panelRef = useRef<HTMLDivElement>(null);

  // Close on click outside or escape
  useEffect(() => {
    if (!selectedMark) return;
    const handleClick = (e: MouseEvent) => {
      if (panelRef.current && !panelRef.current.contains(e.target as Node)) {
        setSelectedMark(null);
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setSelectedMark(null);
    };
    document.addEventListener('mousedown', handleClick);
    document.addEventListener('keydown', handleKey);
    return () => {
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleKey);
    };
  }, [selectedMark, setSelectedMark]);

  const handleEntityClick = useCallback(
    async (entityRef: FsmEntityRef) => {
      setSelectedMark(null);
      // Set tracked entity immediately (FSM null = loading)
      setTrackedEntity({ entityRef, fsm: null });
      try {
        const fsm = await fetchEntityFsm(engineId, queryId, entityRef.id);
        setTrackedEntity({ entityRef, fsm });
      } catch (err) {
        console.error('Failed to fetch entity FSM:', err);
        setTrackedEntity(null);
      }
    },
    [engineId, queryId, setSelectedMark, setTrackedEntity]
  );

  if (!selectedMark) return null;

  const { fsm, activeStateName, screenX, screenY } = selectedMark;
  const stateColorMap = buildStateColorMap(fsmTypes);
  const transitions = fsm.transitions;

  // Collect all related entities across all states
  const hasAnyRelated = transitions.some(
    t => t.related_entities && t.related_entities.length > 0
  );

  // Center the panel on the click point
  const panelStyle: React.CSSProperties = {
    position: 'fixed',
    left: screenX,
    top: screenY,
    transform: 'translate(-50%, -50%)',
    zIndex: 100,
  };

  return (
    <div
      ref={panelRef}
      style={panelStyle}
      className="bg-popover border border-border rounded-md shadow-lg p-2 text-[11px] text-foreground leading-tight w-[300px] max-h-[380px] overflow-y-auto"
    >
      <div className="flex items-center justify-between mb-1">
        <span className="font-semibold text-muted-foreground">
          {fsm.type_name}: {fsm.instance_name || fsm.id}
        </span>
        <button
          onClick={() => setSelectedMark(null)}
          className="rounded-sm hover:bg-accent p-0.5 -mr-0.5 transition-colors text-muted-foreground hover:text-foreground"
        >
          <X className="h-3 w-3" />
        </button>
      </div>
      <div className="flex flex-col gap-px">
        {transitions.slice(0, -1).map((t, i) => {
          const next = transitions[i + 1]!;
          const durationS = next.timestamp - t.timestamp;
          const durationMs = durationS * 1000;
          const isActive = t.name === activeStateName;
          const color = stateColorMap.get(t.name) ?? getColorForKey(t.name);
          const relatedEntities = t.related_entities ?? [];

          const byteUsages = t.usages
            .flatMap(u => u.capacities)
            .filter(([name]) => name === 'bytes')
            .map(([, val]) => (val != null ? Number(val) : 0))
            .filter(v => v > 0);
          const totalBytes = byteUsages.reduce((a, b) => a + b, 0);

          return (
            <div key={i}>
              <div
                className={cn(
                  'flex items-center gap-1 px-0.5 rounded-xs',
                  isActive && 'bg-accent'
                )}
              >
                <span
                  className="w-1.5 h-1.5 rounded-full shrink-0"
                  style={{ backgroundColor: color }}
                />
                <span
                  className={cn(
                    'truncate',
                    isActive ? 'text-foreground font-semibold' : 'text-muted-foreground'
                  )}
                >
                  {t.name}
                </span>
                <span className="ml-auto text-muted-foreground tabular-nums whitespace-nowrap">
                  {formatDuration(durationMs, 1)}
                </span>
                {totalBytes > 0 && (
                  <span className="text-muted-foreground tabular-nums whitespace-nowrap">
                    {formatWithPrefix(totalBytes, 'B', 'Iec', 1)}
                  </span>
                )}
              </div>
              {relatedEntities.length > 0 && (
                <div className="ml-3 mt-px mb-0.5 flex flex-col gap-px">
                  {relatedEntities.map(entity => (
                    <button
                      key={entity.id}
                      onClick={() => handleEntityClick(entity)}
                      className="flex items-center gap-1 px-1 py-0.5 rounded-xs text-left hover:bg-accent transition-colors cursor-pointer"
                    >
                      <span className="text-accent-foreground font-medium truncate">
                        {entity.instance_name || entity.id}
                      </span>
                      <span className="text-muted-foreground text-[10px]">
                        {entity.type_name}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
      {!hasAnyRelated && (
        <div className="text-muted-foreground mt-1 text-center">No related entities</div>
      )}
    </div>
  );
}
