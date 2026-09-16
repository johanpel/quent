// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use super::*;

impl TransitionEvent for TaskEvent {
    fn sequence(&self) -> u16 {
        match self {
            Self::Queueing { seq, .. }
            | Self::Allocating { seq, .. }
            | Self::Loading { seq, .. }
            | Self::Computing { seq, .. }
            | Self::Spilling { seq, .. }
            | Self::Sending { seq, .. }
            | Self::Exit { seq } => *seq,
        }
    }

    fn is_final(&self) -> bool {
        matches!(self, Self::Exit { .. })
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Queueing { .. } => "queueing",
            Self::Allocating { .. } => "allocating",
            Self::Loading { .. } => "loading",
            Self::Computing { .. } => "computing",
            Self::Spilling { .. } => "spilling",
            Self::Sending { .. } => "sending",
            Self::Exit { .. } => "exit",
        }
    }

    fn usages(&self) -> SmallVec<[AnalyzedUsage; 1]> {
        let unit = |resource_id| AnalyzedUsage {
            resource_id,
            capacities: smallvec![CapacityValue::new("unit", 1)],
        };
        let bytes = |resource_id, value| AnalyzedUsage {
            resource_id,
            capacities: smallvec![CapacityValue::new("bytes", value)],
        };
        match self {
            Self::Queueing { .. } | Self::Exit { .. } => SmallVec::new(),
            Self::Allocating { use_thread, .. } => smallvec![unit(use_thread.target)],
            Self::Loading {
                use_thread,
                use_storage_channel,
                use_pcie_channel,
                use_host_memory,
                use_gpu_memory,
                ..
            } => {
                let mut usages = smallvec![unit(use_thread.target)];
                usages.extend(
                    use_storage_channel
                        .iter()
                        .map(|usage| bytes(usage.target, usage.data.bytes)),
                );
                usages.extend(
                    use_pcie_channel
                        .iter()
                        .map(|usage| bytes(usage.target, usage.data.bytes)),
                );
                usages.extend(
                    use_host_memory
                        .iter()
                        .map(|usage| bytes(usage.target, usage.data.bytes)),
                );
                usages.extend(
                    use_gpu_memory
                        .iter()
                        .map(|usage| bytes(usage.target, usage.data.bytes)),
                );
                usages
            }
            Self::Computing {
                use_thread,
                use_host_memory,
                use_gpu_memory,
                ..
            } => {
                let mut usages = smallvec![unit(use_thread.target)];
                usages.extend(
                    use_host_memory
                        .iter()
                        .map(|usage| bytes(usage.target, usage.data.bytes)),
                );
                usages.extend(
                    use_gpu_memory
                        .iter()
                        .map(|usage| bytes(usage.target, usage.data.bytes)),
                );
                usages
            }
            Self::Spilling {
                use_thread,
                use_storage_channel,
                ..
            } => smallvec![
                unit(use_thread.target),
                bytes(use_storage_channel.target, use_storage_channel.data.bytes),
            ],
            Self::Sending {
                use_thread,
                use_network_channel,
                ..
            } => smallvec![
                unit(use_thread.target),
                bytes(use_network_channel.target, use_network_channel.data.bytes),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TaskExecutorThreadUsage;

    #[test]
    fn thread_usage_reserves_one_unit() {
        let thread_id = quent_events::Uuid::from_u128(1);
        let event = TaskEvent::Allocating {
            seq: 0,
            use_thread: quent_events::EntityRef::new(thread_id, TaskExecutorThreadUsage),
        };

        let usages = event.usages();
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].resource_id, thread_id);
        assert_eq!(
            usages[0].capacities.as_slice(),
            &[CapacityValue::new("unit", 1)]
        );
    }
}
