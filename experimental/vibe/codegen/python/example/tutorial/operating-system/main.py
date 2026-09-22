# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import os
import threading

import quent_tutorial_operating_system as quent


def main() -> None:
    with quent.Context() as context:
        process = context.process_observer().handle()
        # This is the native OS process ID, not the Quent entity ID.
        process.started(process={"native_id": os.getpid()})

        thread = context.thread_observer().handle()

        def start_thread() -> None:
            # This is the worker's native OS thread ID, not the Quent entity ID.
            thread.started(
                thread={"native_id": threading.get_native_id()},
                process=process,
            )

        worker = threading.Thread(target=start_thread)
        worker.start()
        worker.join()


if __name__ == "__main__":
    main()
