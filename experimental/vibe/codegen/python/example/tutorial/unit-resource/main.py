# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import quent_tutorial_unit_resource as quent


def main() -> None:
    with quent.Context() as context:
        pool = context.thread_pool_observer().create()
        pool.created()

        thread = context.thread_observer().create()
        thread.registered(pool=pool)

        task = context.task_observer().create()
        task.running(thread={"target": thread, "data": {}})
        task.completed()


if __name__ == "__main__":
    main()
