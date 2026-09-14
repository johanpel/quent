// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'jotai';
import { describe, expect, it } from 'vitest';
import {
  useSelectedOperatorIds,
  useSelectedOperatorsData,
  useOperatorSelectionActions,
} from '@quent/hooks';
import { QueryToolbar } from './QueryToolbar';

function SeedOperatorFilter() {
  const updateOperatorSelection = useOperatorSelectionActions();

  useEffect(() => {
    updateOperatorSelection({
      type: 'add',
      selectionId: 'operator-1',
      label: 'Scan',
      operatorIds: ['operator-1'],
      selectedData: {
        nodeId: 'operator-1',
        label: 'Scan',
        operationType: 'logical',
        statistics: [],
      },
    });
  }, [updateOperatorSelection]);
  return null;
}

function ToolbarHarness() {
  const selectedOperatorIds = useSelectedOperatorIds();
  const selectedOperatorsData = useSelectedOperatorsData();
  const updateOperatorSelection = useOperatorSelectionActions();

  useEffect(() => {
    updateOperatorSelection({
      type: 'add',
      selectionId: 'parent',
      label: 'Parent operator',
      operatorIds: ['parent', 'child'],
      selectedData: {
        nodeId: 'parent',
        label: 'Parent operator',
        operationType: 'logical',
        statistics: [],
      },
    });
  }, [updateOperatorSelection]);

  return (
    <>
      <QueryToolbar />
      <span data-testid="selected-count">{selectedOperatorIds.size}</span>
      <span data-testid="selected-details">{selectedOperatorsData[0]?.nodeId ?? 'none'}</span>
    </>
  );
}

function OperatorSelectionCount() {
  return <span data-testid="operator-count">{useSelectedOperatorIds().size}</span>;
}

function MultiOperatorToolbarHarness() {
  const selectedOperatorIds = useSelectedOperatorIds();
  const selectedOperators = useSelectedOperatorsData();
  const updateOperatorSelection = useOperatorSelectionActions();

  useEffect(() => {
    for (let index = 0; index < 5; index += 1) {
      const number = index + 1;
      const id = `operator-${number}`;
      updateOperatorSelection({
        type: 'add',
        selectionId: id,
        label: `Operator ${number}`,
        operatorIds: [id],
        selectedData: {
          nodeId: id,
          label: `Operator ${number}`,
          operationType: 'physical',
          statistics: [],
        },
      });
    }
  }, [updateOperatorSelection]);

  return (
    <>
      <QueryToolbar />
      <span data-testid="selected-count">{selectedOperatorIds.size}</span>
      <span data-testid="selected-data-count">{selectedOperators.length}</span>
    </>
  );
}

function TwoOperatorToolbarHarness() {
  const selectedOperators = useSelectedOperatorsData();
  const updateOperatorSelection = useOperatorSelectionActions();

  useEffect(() => {
    updateOperatorSelection({
      type: 'add',
      selectionId: 'scan',
      label: 'Scan',
      operatorIds: ['scan'],
      selectedData: {
        nodeId: 'scan',
        label: 'Scan',
        operationType: 'scan',
        statistics: [],
      },
    });
    updateOperatorSelection({
      type: 'add',
      selectionId: 'join',
      label: 'Join',
      operatorIds: ['join'],
      selectedData: {
        nodeId: 'join',
        label: 'Join',
        operationType: 'join',
        statistics: [],
      },
    });
  }, [updateOperatorSelection]);

  return (
    <>
      <QueryToolbar />
      <span data-testid="selected-data-ids">
        {selectedOperators.map(operator => operator.nodeId).join(',')}
      </span>
    </>
  );
}

describe('QueryToolbar', () => {
  it('shows custom resource filters before the active operator filter', async () => {
    render(
      <Provider>
        <SeedOperatorFilter />
        <QueryToolbar filters={<input aria-label="Filter resources" />} />
      </Provider>
    );

    const operatorFilter = await screen.findByText('Scan');
    const resourceFilters = screen.getByRole('textbox', { name: 'Filter resources' });
    expect(resourceFilters.compareDocumentPosition(operatorFilter)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(screen.queryByText('No filters')).not.toBeInTheDocument();
  });

  it('clears only the operator filter', async () => {
    const user = userEvent.setup();
    render(
      <Provider>
        <SeedOperatorFilter />
        <OperatorSelectionCount />
        <QueryToolbar filters={<input aria-label="Filter resources" value="id:gpu-0" readOnly />} />
      </Provider>
    );

    await user.click(await screen.findByRole('button', { name: 'Clear all operator filters' }));
    expect(screen.getByTestId('operator-count')).toHaveTextContent('0');
    expect(screen.getByRole('textbox', { name: 'Filter resources' })).toHaveValue('id:gpu-0');
  });

  it('clears the full operator selection and pinned details', async () => {
    const user = userEvent.setup();
    render(
      <Provider>
        <ToolbarHarness />
      </Provider>
    );

    await user.click(await screen.findByRole('button', { name: 'Clear all operator filters' }));

    expect(screen.getByTestId('selected-count')).toHaveTextContent('0');
    expect(screen.getByTestId('selected-details')).toHaveTextContent('none');
  });

  it('caps badges and supports individual and bulk clearing', async () => {
    render(
      <Provider>
        <MultiOperatorToolbarHarness />
      </Provider>
    );

    expect(await screen.findByText('Operator 1')).toBeInTheDocument();
    expect(screen.getByText('Operator 2')).toBeInTheDocument();
    expect(screen.getByText('Operator 3')).toBeInTheDocument();
    expect(screen.queryByText('Operator 4')).not.toBeInTheDocument();
    expect(screen.getByText('and 2 more')).toBeInTheDocument();
    expect(screen.getByText('and 2 more')).toHaveAttribute('title', 'Operator 4, Operator 5');
    expect(screen.getByTestId('operator-filter-badges')).toHaveClass('max-w-[40%]');

    fireEvent.click(screen.getByRole('button', { name: 'Remove Operator 2' }));

    expect(screen.getByTestId('selected-count')).toHaveTextContent('4');
    expect(screen.getByTestId('selected-data-count')).toHaveTextContent('4');
    expect(screen.getByText('Operator 4')).toBeInTheDocument();
    expect(screen.getByText('and 1 more')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Clear all operator filters' }));

    expect(screen.getByTestId('selected-count')).toHaveTextContent('0');
    expect(screen.getByTestId('selected-data-count')).toHaveTextContent('0');
    expect(screen.getByText('No filters')).toBeInTheDocument();
  });

  it('keeps remaining operator details after removing the last-clicked badge', async () => {
    render(
      <Provider>
        <TwoOperatorToolbarHarness />
      </Provider>
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Remove Join' }));

    expect(screen.getByTestId('selected-data-ids')).toHaveTextContent('scan');
    expect(screen.getByText('Scan')).toBeInTheDocument();
    expect(screen.queryByText('Join')).not.toBeInTheDocument();
  });
});
