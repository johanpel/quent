# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import quent_tutorial_finite_state_machine as quent


def main() -> None:
    with quent.Context() as context:
        job = context.job_observer().create()
        job.queued()
        job.loading_input()
        job.running()
        job.completed()


if __name__ == "__main__":
    main()
