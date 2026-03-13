import { useState, useCallback } from 'react';

/* getter/setter for tracking expanded IDs in the resource tree */
export function useExpandedIds(initialId?: string) {
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => {
    return initialId ? new Set([initialId]) : new Set();
  });

  const handleExpandChange = useCallback((itemId: string, isExpanded: boolean) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      if (isExpanded) {
        next.add(itemId);
      } else {
        next.delete(itemId);
      }
      return next;
    });
  }, []);

  /** Expand all provided IDs (additive — does not collapse anything). */
  const expandAll = useCallback((ids: Iterable<string>) => {
    setExpandedIds(prev => {
      const next = new Set(prev);
      for (const id of ids) {
        next.add(id);
      }
      return next;
    });
  }, []);

  return { expandedIds, handleExpandChange, expandAll } as const;
}
