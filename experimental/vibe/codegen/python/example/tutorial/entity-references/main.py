# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import quent_tutorial_entity_references as quent


def main() -> None:
    with quent.Context() as context:
        worker = context.worker_observer().create()
        worker.registered()

        task = context.task_observer().create()
        task.started(worker=worker)
        task.ended()


if __name__ == "__main__":
    main()
