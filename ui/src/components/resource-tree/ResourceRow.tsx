import { useAtom } from 'jotai';
import { Resource } from '~quent/types/Resource';
import { FsmTypeSelector } from './FsmTypeSelector';
import { groupFsmFiltersAtom } from '@/atoms/timeline';

interface ResourceRowProps {
  resource: Resource;
  id: string;
  availableFsmTypes?: string[];
}

export const ResourceRow = ({ resource, id, availableFsmTypes }: ResourceRowProps): React.ReactNode => {
  const [fsmFilters, setFsmFilters] = useAtom(groupFsmFiltersAtom);

  const handleFsmChange = (_itemId: string, fsmType: string | null) => {
    setFsmFilters(prev => new Map(prev).set(id, fsmType));
  };

  return (
    <div>
      <div>
        <span className="text-xs font-bold">
          {resource.instance_name}{' '}
          {resource.type_name !== resource.instance_name && resource.type_name
            ? `(${resource.type_name})`
            : ''}
        </span>
      </div>
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
