// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#[test]
fn fsm_typestate_transitions_compile() {
    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/valid_chain.rs");
    tests.compile_fail("tests/ui/skipped_transition.rs");
    tests.compile_fail("tests/ui/final_transition.rs");
    tests.compile_fail("tests/ui/reused_handle.rs");
}
