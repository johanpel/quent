import { useAtom } from 'jotai';
import { ResourceGroup } from '~quent/types/ResourceGroup';
import { ResourceTypeSelector } from './ResourceTypeSelector';
import { FsmTypeSelector } from './FsmTypeSelector';
import { groupFsmFiltersAtom } from '@/atoms/timeline';

interface ResourceGroupRowProps {
  group: ResourceGroup;
  id: string;
  availableResourceTypes?: string[];
  selectedType?: string;
  onTypeChange?: (itemId: string, type: string) => void;
  availableFsmTypes?: string[];
  verbose?: boolean;
}

export const ResourceGroupRow = ({
  group,
  id,
  availableResourceTypes,
  selectedType,
  onTypeChange,
  availableFsmTypes,
}: ResourceGroupRowProps): React.ReactNode => {
  const hasMultipleChildTypes = (availableResourceTypes?.length ?? 0) > 1;

  const [fsmFilters, setFsmFilters] = useAtom(groupFsmFiltersAtom);

  const handleFsmChange = (_itemId: string, fsmType: string | null) => {
    setFsmFilters(prev => new Map(prev).set(id, fsmType));
  };

  return (
    <div className="flex items-baseline gap-x-2 gap-y-0 flex-wrap">
      <span className="text-xs font-bold">{group.instance_name}</span>
      {hasMultipleChildTypes && selectedType && onTypeChange && availableResourceTypes ? (
        <ResourceTypeSelector
          id={id}
          selectedType={selectedType}
          availableResourceTypes={availableResourceTypes}
          onTypeChange={onTypeChange}
        />
      ) : selectedType ? (
        <span className="text-[10px] text-muted-foreground">{selectedType}</span>
      ) : null}
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
