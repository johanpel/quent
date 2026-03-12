import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
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
      className={cn('flex items-center gap-1.5', className)}
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
    >
      <label id={`fsm-select-label-${id}`} className="text-[10px] text-muted-foreground shrink-0">
        FSM:
      </label>
      <Select
        value={selectedFsm ?? '__all__'}
        onValueChange={value => onFsmChange(id, value === '__all__' ? null : value)}
      >
        <SelectTrigger
          id={`fsm-select-${id}`}
          aria-labelledby={`fsm-select-label-${id}`}
          className={cn(
            'h-auto w-auto min-w-0 max-w-80 border-0 border-b border-dashed border-muted-foreground/60 rounded-none bg-transparent px-0 py-px text-[10px] shadow-none cursor-pointer',
            'focus:ring-0 focus:ring-offset-0 focus-visible:ring-0 focus-visible:ring-offset-0',
            'data-[placeholder]:text-muted-foreground',
            '[&>svg]:h-3 [&>svg]:w-3 [&>svg]:shrink-0 [&>svg]:translate-y-px [&>svg]:opacity-70'
          )}
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent
          position="popper"
          className="max-h-[--radix-select-content-available-height] min-w-[var(--radix-select-trigger-width)]"
        >
          <SelectItem value="__all__" className="text-[10px] py-1.5 pl-8 pr-2 cursor-pointer">
            All
          </SelectItem>
          {availableFsmTypes.map(fsmType => (
            <SelectItem
              key={fsmType}
              value={fsmType}
              className="text-[10px] py-1.5 pl-8 pr-2 cursor-pointer"
            >
              {fsmType}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
};
