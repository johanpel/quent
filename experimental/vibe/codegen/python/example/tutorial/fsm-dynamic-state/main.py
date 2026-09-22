# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import sys

import quent_tutorial_fsm_dynamic_state as quent


def prepare_job(
    job: quent.JobQueuedHandle, restore_from_checkpoint: bool
) -> quent.JobDynamicFsmHandle:
    if restore_from_checkpoint:
        return job.restoring_checkpoint().into_dynamic()
    return job.loading_input().into_dynamic()


def main() -> None:
    restore_from_checkpoint = "--restore" in sys.argv
    with quent.Context() as context:
        queued = context.job_observer().handle().queued()
        job = prepare_job(queued, restore_from_checkpoint)
        job.running()
        running = job.try_into_running()
        running.completed()


if __name__ == "__main__":
    main()
