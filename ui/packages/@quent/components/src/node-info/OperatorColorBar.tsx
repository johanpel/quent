// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { COLOR_REGISTRY_KEYS, useColorResolver } from '@quent/hooks';
import { cn } from '@quent/utils';

export const OperatorColorBar = ({
  operationType,
  className,
}: {
  operationType: string;
  className?: string;
}) => {
  const resolveOperatorTypeColor = useColorResolver(COLOR_REGISTRY_KEYS.OPERATOR_TYPES);
  return (
    <span
      aria-hidden
      data-testid="operator-color-bar"
      data-operation-type={operationType}
      className={cn('shrink-0 rounded-full', className)}
      style={{ backgroundColor: resolveOperatorTypeColor(operationType) }}
    />
  );
};
