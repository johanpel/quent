import { useAtom, useAtomValue, useSetAtom } from 'jotai';
import { X, Maximize2, Filter, Settings, Eye } from 'lucide-react';
import { selectedNodeIdsAtom, selectedOperatorLabelAtom } from '@/atoms/dag';
import { hideTasksAtom, zoomRangeAtom, debouncedZoomRangeAtom, trackedEntityAtom } from '@/atoms/timeline';
import { Popover, PopoverTrigger, PopoverContent } from '@/components/ui/popover';

export function TimelineToolbar({ durationSeconds }: { durationSeconds: number }) {
  const operatorLabel = useAtomValue(selectedOperatorLabelAtom);
  const setSelectedNodeIds = useSetAtom(selectedNodeIdsAtom);
  const setSelectedOperatorLabel = useSetAtom(selectedOperatorLabelAtom);
  const [hideTasks, setHideTasks] = useAtom(hideTasksAtom);
  const [trackedEntity, setTrackedEntity] = useAtom(trackedEntityAtom);
  const setZoomRange = useSetAtom(zoomRangeAtom);
  const setDebouncedZoomRange = useSetAtom(debouncedZoomRangeAtom);

  const clearOperator = () => {
    setSelectedNodeIds(new Set());
    setSelectedOperatorLabel(null);
  };

  const resetZoom = () => {
    const full = { start: 0, end: durationSeconds };
    setZoomRange(full);
    setDebouncedZoomRange(full);
  };

  return (
    <div className="flex items-center gap-4 px-3 py-1 border-b border-border text-xs text-muted-foreground shrink-0 h-8">
      {/* Operator filter */}
      <div className="flex items-center gap-1.5">
        <Filter className="h-3 w-3" />
        {operatorLabel ? (
          <span className="inline-flex items-center gap-1.5 rounded-md bg-primary text-primary-foreground px-2.5 py-1 font-bold text-sm shadow-sm">
            {operatorLabel}
            <button
              onClick={clearOperator}
              className="rounded-sm hover:bg-primary-foreground/20 p-0.5 -mr-1 transition-colors"
            >
              <X className="h-3 w-3" />
            </button>
          </span>
        ) : (
          <span>No filters</span>
        )}
      </div>

      {/* Tracked entity */}
      {trackedEntity && (
        <div className="flex items-center gap-1.5">
          <Eye className="h-3 w-3" />
          <span className="inline-flex items-center gap-1.5 rounded-md bg-accent text-accent-foreground px-2.5 py-1 text-sm shadow-sm">
            <span className="text-muted-foreground font-normal">{trackedEntity.entityRef.type_name}</span>
            <span className="font-bold">{trackedEntity.entityRef.instance_name || trackedEntity.entityRef.id}</span>
            {!trackedEntity.fsm && (
              <span className="text-muted-foreground font-normal text-[10px]">loading…</span>
            )}
            <button
              onClick={() => setTrackedEntity(null)}
              className="rounded-sm hover:bg-accent-foreground/20 p-0.5 -mr-1 transition-colors"
            >
              <X className="h-3 w-3" />
            </button>
          </span>
        </div>
      )}

      <div className="flex-1" />

      {/* Zoom reset */}
      <button
        onClick={resetZoom}
        className="inline-flex items-center gap-1 rounded-sm px-1.5 py-0.5 hover:bg-accent hover:text-accent-foreground transition-colors"
        title="Reset zoom"
      >
        <Maximize2 className="h-3 w-3" />
        <span>Reset zoom</span>
      </button>

      <div className="h-3 w-px bg-border" />

      {/* Settings popover */}
      <Popover>
        <PopoverTrigger asChild>
          <button
            className="inline-flex items-center rounded-sm p-0.5 hover:bg-accent hover:text-accent-foreground transition-colors"
            title="Timeline settings"
          >
            <Settings className="h-3.5 w-3.5" />
          </button>
        </PopoverTrigger>
        <PopoverContent className="text-xs">
          <label className="flex items-center gap-2 cursor-pointer select-none">
            <input
              type="checkbox"
              checked={hideTasks}
              onChange={e => setHideTasks(e.target.checked)}
              className="h-3 w-3 rounded-sm accent-primary cursor-pointer"
            />
            <span>Hide tasks</span>
          </label>
        </PopoverContent>
      </Popover>
    </div>
  );
}
