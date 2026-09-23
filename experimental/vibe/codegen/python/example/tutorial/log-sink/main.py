# SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
# SPDX-License-Identifier: Apache-2.0

import quent_tutorial_log_sink as quent


def main() -> None:
    with quent.Context() as context:
        log = context.app_log_observer().handle()

        # A logging API would typically capture the file, line, and any other
        # call-site context required by the schema.
        log.info(
            message="application started",
            target="startup",
            file=__file__,
            line=13,
            module=__name__,
            thread_name="main",
        )
        log.warning(
            message="retrying request",
            target="network",
            file=__file__,
            line=21,
            module=__name__,
            thread_name="main",
            category="transient",
        )


if __name__ == "__main__":
    main()
