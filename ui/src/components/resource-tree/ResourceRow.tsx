import { useAtom } from 'jotai';
import { Resource } from '~quent/types/Resource';
import { FsmTypeSelector } from './FsmTypeSelector';
import { groupFsmFiltersAtom } from '@/atoms/timeline';

interface ResourceRowProps {
  resource: Resource;
  id: string;
  availableFsmTypes?: string[];
}

export const ResourceRow = ({
  resource,
  id,
  availableFsmTypes,
}: ResourceRowProps): React.ReactNode => {
  const [fsmFilters, setFsmFilters] = useAtom(groupFsmFiltersAtom);

  const handleFsmChange = (_itemId: string, fsmType: string | null) => {
    setFsmFilters(prev => new Map(prev).set(id, fsmType));
  };

  return (
    <div className="flex items-baseline gap-x-2 gap-y-0 flex-wrap">
      <span className="text-xs font-bold">{resource.instance_name}</span>
      {resource.type_name !== resource.instance_name && resource.type_name && (
        <span className="text-[10px] text-muted-foreground">{resource.type_name}</span>
      )}
      {availableFsmTypes && availableFsmTypes.length === 1 && (
        <span className="text-[10px] text-muted-foreground">FSM: {availableFsmTypes[0]}</span>
      )}
      {availableFsmTypes && availableFsmTypes.length > 1 && (
        <FsmTypeSelector
          id={id}
          selectedFsm={fsmFilters.has(id) ? (fsmFilters.get(id) ?? null) : null}
          availableFsmTypes={availableFsmTypes}
          onFsmChange={handleFsmChange}
        />
      )}
    </div>
  );
};
