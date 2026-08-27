// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Data batch FSM for the simulator.

use quent_model::{fsm, state};
use uuid::Uuid;

state! {
    Initialized {
        attributes: {
            operator_id: Uuid,
        },
    }
}

state! {
    InStorage {
        usages: {
            use_storage: quent_stdlib::memory::Memory,
        },
    }
}

state! {
    LoadingToHostMemory {
        usages: {
            use_storage_to_host: quent_stdlib::channel::Channel,
            use_storage: quent_stdlib::memory::Memory,
        },
    }
}

state! {
    InHostMemory {
        usages: {
            use_host_memory: quent_stdlib::memory::Memory,
        },
    }
}

state! {
    LoadingToGpuMemory {
        usages: {
            use_host_mem_to_gpu: quent_stdlib::channel::Channel,
            use_host_memory: quent_stdlib::memory::Memory,
        },
    }
}

state! {
    InGpuMemory {
        usages: {
            use_gpu_memory: quent_stdlib::memory::Memory,
        },
    }
}

state! {
    SpillingToHostMemory {
        usages: {
            use_gpu_to_host_mem: quent_stdlib::channel::Channel,
            use_gpu_memory: quent_stdlib::memory::Memory,
        },
    }
}

state! {
    SpillingToStorage {
        usages: {
            use_host_to_storage: quent_stdlib::channel::Channel,
            use_host_memory: quent_stdlib::memory::Memory,
        },
    }
}

fsm! {
    DataBatch {
        states: {
            initialized: Initialized,
            in_storage: InStorage,
            loading_to_host_memory: LoadingToHostMemory,
            in_host_memory: InHostMemory,
            loading_to_gpu_memory: LoadingToGpuMemory,
            in_gpu_memory: InGpuMemory,
            spilling_to_host_memory: SpillingToHostMemory,
            spilling_to_storage: SpillingToStorage,
        },
        entry: initialized,
        exit_from: { initialized, in_storage, in_host_memory, in_gpu_memory },
        transitions: {
            initialized => in_storage,
            initialized => in_host_memory,
            initialized => in_gpu_memory,
            in_storage => loading_to_host_memory,
            loading_to_host_memory => in_host_memory,
            in_host_memory => loading_to_gpu_memory,
            in_host_memory => spilling_to_storage,
            loading_to_gpu_memory => in_gpu_memory,
            in_gpu_memory => spilling_to_host_memory,
            spilling_to_host_memory => in_host_memory,
            spilling_to_storage => in_storage,
        },
    }
}
