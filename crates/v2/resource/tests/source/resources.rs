// SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]

use quent_v2_resource::{Capacity, Fixed, Occupancy, Rate, Resizeable, Unbounded, resource};

// Single capacity, Occupancy kind.

resource! {
    pub SingleOccupancy {
        a: Capacity<u64, Occupancy, Fixed>,
    }
}

resource! {
    pub SingleOccupancyResize {
        a: Capacity<u64, Occupancy, Resizeable>,
    }
}

resource! {
    pub SingleOccupancyUnbound {
        a: Capacity<u64, Occupancy, Unbounded>,
    }
}

// Single capacity, Rate kind.

resource! {
    pub SingleRate {
        a: Capacity<u64, Rate, Fixed>,
    }
}

resource! {
    pub SingleRateResize {
        a: Capacity<u64, Rate, Resizeable>,
    }
}

resource! {
    pub SingleRateUnbound {
        a: Capacity<u64, Rate, Unbounded>,
    }
}

resource! {
    /// A documented resource for testing docstring propagation.
    /// Second line.
    pub DocumentedResource {
        a: Capacity<u64, Occupancy, Fixed>,
    }
}
