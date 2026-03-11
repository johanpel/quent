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
  const hasFsmTypes = (availableFsmTypes?.length ?? 0) >= 1;

  const [fsmFilters, setFsmFilters] = useAtom(groupFsmFiltersAtom);

  const handleFsmChange = (_itemId: string, fsmType: string | null) => {
    setFsmFilters(prev => new Map(prev).set(id, fsmType));
  };

  return (
    <div>
      <div>
        <span className="text-xs font-bold">{group.instance_name}</span>
      </div>
      {hasMultipleChildTypes && selectedType && onTypeChange && availableResourceTypes && (
        <ResourceTypeSelector
          id={id}
          selectedType={selectedType}
          availableResourceTypes={availableResourceTypes}
          onTypeChange={onTypeChange}
          className="mt-1"
        />
      )}
      {availableFsmTypes && availableFsmTypes.length === 1 && (
        <div className="text-xs text-muted-foreground mt-1">{availableFsmTypes[0]}</div>
      )}
      {availableFsmTypes && availableFsmTypes.length > 1 && (
        <FsmTypeSelector
          id={id}
          selectedFsm={fsmFilters.has(id) ? (fsmFilters.get(id) ?? null) : null}
          availableFsmTypes={availableFsmTypes}
          onFsmChange={handleFsmChange}
          className="mt-1"
        />
      )}
    </div>
  );
};
