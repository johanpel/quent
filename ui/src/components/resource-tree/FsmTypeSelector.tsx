import { cn } from '@/lib/utils';

interface FsmTypeSelectorProps {
  id: string;
  selectedFsm: string | null;
  availableFsmTypes: string[];
  onFsmChange: (itemId: string, fsmType: string | null) => void;
  className?: string;
}

export const FsmTypeSelector = ({
  id,
  selectedFsm,
  availableFsmTypes,
  onFsmChange,
  className,
}: FsmTypeSelectorProps): React.ReactNode => {
  return (
    <div
      className={cn('flex items-center gap-2', className)}
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
    >
      <label htmlFor={`fsm-select-${id}`} className="text-xs text-muted-foreground">
        FSM:
      </label>
      <select
        id={`fsm-select-${id}`}
        value={selectedFsm ?? '__all__'}
        onChange={e => {
          e.stopPropagation();
          onFsmChange(id, e.target.value === '__all__' ? null : e.target.value);
        }}
        className="text-xs bg-background border border-border rounded px-1 py-0.5 text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
      >
        <option value="__all__">All</option>
        {availableFsmTypes.map(fsmType => (
          <option key={fsmType} value={fsmType}>
            {fsmType}
          </option>
        ))}
      </select>
    </div>
  );
};
