# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import quent_tutorial_fsm_self_loop as quent


def main() -> None:
    with quent.Context() as context:
        task = context.task_observer().create()
        task.running(items_processed=0)
        task.running(items_processed=64)
        task.paused()
        task.running(items_processed=128)
        task.completed()


if __name__ == "__main__":
    main()
