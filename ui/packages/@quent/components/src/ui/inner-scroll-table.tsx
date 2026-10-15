// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import * as React from 'react';

import { cn } from '@quent/utils';
import { Table, type TableProps } from './table';
import { thinScrollbarClass } from './thin-scroll';

export interface InnerScrollTableProps extends TableProps {
  stickyLeftColumns?: number;
  stickyRightColumns?: number;
}

const useIsomorphicLayoutEffect =
  typeof window === 'undefined' ? React.useEffect : React.useLayoutEffect;

const innerScrollTableClassName = cn(
  '[&_thead_th]:sticky [&_thead_th]:top-0 [&_thead_th]:z-30 [&_thead_th]:bg-card',
  '[&_thead_tr]:border-b-0',
  '[&_[data-inner-scroll-sticky]]:sticky [&_[data-inner-scroll-sticky]]:z-10',
  '[&_[data-inner-scroll-sticky]]:bg-card',
  '[&_thead_[data-inner-scroll-sticky]]:z-40',
  '[&_thead_th]:shadow-[inset_0_-1px_0_hsl(var(--border))]',
  '[&_[data-inner-scroll-sticky-left]]:shadow-[inset_-1px_0_hsl(var(--border))]',
  '[&_[data-inner-scroll-sticky-right]]:shadow-[inset_1px_0_hsl(var(--border))]',
  '[&_thead_[data-inner-scroll-sticky-left]]:shadow-[inset_-1px_0_hsl(var(--border)),inset_0_-1px_0_hsl(var(--border))]',
  '[&_thead_[data-inner-scroll-sticky-right]]:shadow-[inset_1px_0_hsl(var(--border)),inset_0_-1px_0_hsl(var(--border))]',
  '[&_tbody_tr[data-inner-scroll-data-row]]:transition-none',
  '[&_tbody_tr[data-inner-scroll-data-row]_[data-inner-scroll-sticky]]:transition-none',
  '[&_tbody_tr[data-inner-scroll-data-row]:hover]:bg-[color-mix(in_srgb,hsl(var(--muted))_70%,hsl(var(--card)))]',
  '[&_tbody_tr[data-inner-scroll-data-row]:hover>[data-inner-scroll-sticky]]:bg-[color-mix(in_srgb,hsl(var(--muted))_70%,hsl(var(--card)))]'
);

function columnCount(row: HTMLTableRowElement) {
  return Array.from(row.cells).reduce((count, cell) => count + cell.colSpan, 0);
}

function layoutCells(table: HTMLTableElement) {
  const rows = Array.from(table.rows).filter(row =>
    Array.from(row.cells).every(cell => cell.colSpan === 1)
  );
  const row = rows.reduce<HTMLTableRowElement | undefined>(
    (best, candidate) => (!best || candidate.cells.length > best.cells.length ? candidate : best),
    undefined
  );
  return row ? Array.from(row.cells) : [];
}

function setRef<T>(ref: React.ForwardedRef<T>, value: T | null) {
  if (typeof ref === 'function') {
    ref(value);
  } else if (ref) {
    ref.current = value;
  }
}

export const InnerScrollTable = React.forwardRef<HTMLTableElement, InnerScrollTableProps>(
  (
    {
      children,
      className,
      containerClassName,
      stickyLeftColumns = 0,
      stickyRightColumns = 0,
      ...props
    },
    forwardedRef
  ) => {
    const tableRef = React.useRef<HTMLTableElement>(null);
    const ref = React.useCallback(
      (table: HTMLTableElement | null) => {
        tableRef.current = table;
        setRef(forwardedRef, table);
      },
      [forwardedRef]
    );

    useIsomorphicLayoutEffect(() => {
      const table = tableRef.current;
      if (!table) {
        return;
      }

      const updateStickyColumns = () => {
        const cells = layoutCells(table);
        const columns = cells.length;
        const leftCount = Math.min(Math.max(0, Math.floor(stickyLeftColumns)), columns);
        const rightCount = Math.min(
          Math.max(0, Math.floor(stickyRightColumns)),
          columns - leftCount
        );
        const widths = cells.map(cell => cell.getBoundingClientRect().width);
        const leftOffsets = widths.map((_, index) =>
          widths.slice(0, index).reduce((sum, width) => sum + width, 0)
        );
        const rightOffsets = widths.map((_, index) =>
          widths.slice(index + 1).reduce((sum, width) => sum + width, 0)
        );

        for (const row of Array.from(table.rows)) {
          const rowCells = Array.from(row.cells);
          row.removeAttribute('data-inner-scroll-data-row');
          if (
            row.parentElement?.tagName === 'TBODY' &&
            rowCells.every(cell => cell.colSpan === 1) &&
            columnCount(row) === columns
          ) {
            row.setAttribute('data-inner-scroll-data-row', '');
          }

          let columnIndex = 0;
          for (const cell of rowCells) {
            cell.removeAttribute('data-inner-scroll-sticky');
            cell.removeAttribute('data-inner-scroll-sticky-left');
            cell.removeAttribute('data-inner-scroll-sticky-right');
            cell.style.removeProperty('left');
            cell.style.removeProperty('right');

            if (cell.colSpan === 1 && columnIndex < leftCount) {
              cell.setAttribute('data-inner-scroll-sticky', '');
              cell.setAttribute('data-inner-scroll-sticky-left', '');
              cell.style.left = `${leftOffsets[columnIndex]}px`;
            } else if (cell.colSpan === 1 && columnIndex >= columns - rightCount) {
              cell.setAttribute('data-inner-scroll-sticky', '');
              cell.setAttribute('data-inner-scroll-sticky-right', '');
              cell.style.right = `${rightOffsets[columnIndex]}px`;
            }
            columnIndex += cell.colSpan;
          }
        }
      };

      updateStickyColumns();
      window.addEventListener('resize', updateStickyColumns);
      const observer =
        typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(updateStickyColumns);
      observer?.observe(table);
      for (const cell of layoutCells(table)) {
        observer?.observe(cell);
      }
      const mutationObserver =
        typeof MutationObserver === 'undefined'
          ? undefined
          : new MutationObserver(updateStickyColumns);
      mutationObserver?.observe(table, { childList: true, subtree: true });
      return () => {
        mutationObserver?.disconnect();
        observer?.disconnect();
        window.removeEventListener('resize', updateStickyColumns);
      };
    }, [stickyLeftColumns, stickyRightColumns]);

    return (
      <Table
        ref={ref}
        data-slot="inner-scroll-table"
        className={cn(innerScrollTableClassName, className)}
        containerClassName={cn(
          'overflow-auto overscroll-none',
          thinScrollbarClass,
          containerClassName
        )}
        {...props}
      >
        {children}
      </Table>
    );
  }
);
InnerScrollTable.displayName = 'InnerScrollTable';
