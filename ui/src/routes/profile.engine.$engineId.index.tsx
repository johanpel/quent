// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

import { createFileRoute } from '@tanstack/react-router';
import { QuerySelectionPage } from '@/pages/QuerySelectionPage';

export const Route = createFileRoute('/profile/engine/$engineId/')({
  component: ProfileIndex,
});

function ProfileIndex() {
  const { engineId } = Route.useParams();
  return <QuerySelectionPage engineId={engineId} />;
}
