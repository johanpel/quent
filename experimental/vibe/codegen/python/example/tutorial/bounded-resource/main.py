# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import quent_tutorial_bounded_resource as quent


def main() -> None:
    with quent.Context() as context:
        memory = context.memory_observer().create()
        memory.resized(limits={"bytes": 8_000_000_000})

        task = context.task_observer().create()
        task.running(
            memory={
                "target": memory,
                "data": {"bytes": 512_000_000},
            }
        )
        task.completed()


if __name__ == "__main__":
    main()
