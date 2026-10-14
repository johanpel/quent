// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { createFileRoute, redirect } from '@tanstack/react-router';

export const Route = createFileRoute('/profile/')({
  beforeLoad: () => {
    throw redirect({ to: '/' });
  },
});
