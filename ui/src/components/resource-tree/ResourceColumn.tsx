import { ResourceGroup } from '~quent/types/ResourceGroup';
import { Resource } from '~quent/types/Resource';
import type { QueryEntities } from '~quent/types/QueryEntities';
import { cn } from '@/lib/utils';
import { TreeTableItem } from './types';
import { ResourceGroupRow } from './ResourceGroupRow';
import { ResourceRow } from './ResourceRow';

type ResourceColumnProps = {
  item: TreeTableItem;
  isExpanded?: boolean;
  selectedType: string;
  onTypeChange: (itemId: string, type: string) => void;
  entities?: QueryEntities;
  className?: string;
  verbose?: boolean;
};

export function ResourceColumn({
  item,
  isExpanded,
  selectedType,
  onTypeChange,
  entities,
  className,
}: ResourceColumnProps): React.ReactNode {
  const isGroup = !!item?.children?.length;

  // Expanded groups: compact single-line display.
  if (isGroup && isExpanded) {
    const group = item.entity as ResourceGroup;
    return (
      <div className={cn('text-foreground flex truncate items-center', className)}>
        <div>{item.icon && <item.icon className="h-3 w-3 shrink-0 mr-2" />}</div>
        <span className="text-xs font-bold">{group.instance_name}</span>
      </div>
    );
  }

  const entityTypeName = item.entity && 'type_name' in item.entity ? item.entity.type_name as string : undefined;
  const leafResourceTypeDecl = entityTypeName ? entities?.resource_types[entityTypeName] : undefined;
  const resourceTypeDecl = entities?.resource_types[selectedType];
  const availableFsmTypes = isGroup ? resourceTypeDecl?.used_by : leafResourceTypeDecl?.used_by;

  return (
    <div className={cn('text-foreground flex truncate items-center py-0.5', className)}>
      <div>{item.icon && <item.icon className="h-4 w-4 shrink-0 mr-4" />}</div>
      <div>
        {isGroup ? (
          <ResourceGroupRow
            group={item.entity as ResourceGroup}
            id={item.id}
            availableResourceTypes={item.availableResourceTypes}
            selectedType={selectedType}
            onTypeChange={onTypeChange}
            availableFsmTypes={availableFsmTypes}
          />
        ) : (
          <ResourceRow
            resource={item.entity as Resource}
            id={item.id}
            availableFsmTypes={availableFsmTypes}
          />
        )}
      </div>
    </div>
  );
}
