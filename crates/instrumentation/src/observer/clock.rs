// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::OnceLock;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::time::Instant;

const RESYNC_INTERVAL_NS: u64 = 500_000_000;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
const CALIBRATION_INTERVAL_NS: u64 = 10_000_000;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
const CALIBRATION_TRIALS: usize = 5;
const MAX_SYNC_LAG_TICKS: u64 = 50_000;

#[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
const USE_COUNTER: bool = !quent_time::timestamp_override_enabled();
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
const USE_COUNTER: bool = false;

#[inline(always)]
pub(super) fn capture() -> u64 {
    if USE_COUNTER {
        read_counter()
    } else {
        quent_time::timestamp()
    }
}

pub(super) struct TimestampConverter {
    nanoseconds_per_tick: f64,
    base_counter: u64,
    base_time: u64,
    resync_interval_ticks: i64,
}

impl TimestampConverter {
    pub(super) fn new() -> Self {
        if !USE_COUNTER {
            return Self {
                nanoseconds_per_tick: 1.0,
                base_counter: 0,
                base_time: 0,
                resync_interval_ticks: i64::MAX,
            };
        }

        static NANOS_PER_TICK: OnceLock<f64> = OnceLock::new();
        let nanoseconds_per_tick = *NANOS_PER_TICK.get_or_init(calibrate);
        let mut converter = Self {
            nanoseconds_per_tick,
            base_counter: 0,
            base_time: 0,
            resync_interval_ticks: (RESYNC_INTERVAL_NS as f64 / nanoseconds_per_tick) as i64,
        };
        converter.resync();
        converter
    }

    #[inline]
    pub(super) fn convert(&mut self, captured: u64) -> u64 {
        if !USE_COUNTER {
            return captured;
        }

        let mut difference = captured.wrapping_sub(self.base_counter) as i64;
        if difference > self.resync_interval_ticks {
            self.resync();
            difference = captured.wrapping_sub(self.base_counter) as i64;
        }
        self.base_time
            .saturating_add_signed((difference as f64 * self.nanoseconds_per_tick) as i64)
    }

    #[cold]
    fn resync(&mut self) {
        for _ in 0..4 {
            let before = read_counter();
            let wall_time = quent_time::timestamp();
            let after = read_counter();
            if after.wrapping_sub(before) <= MAX_SYNC_LAG_TICKS {
                self.base_counter = fast_average(before, after);
                self.base_time = wall_time;
                return;
            }
        }

        let before = read_counter();
        self.base_time = quent_time::timestamp();
        self.base_counter = fast_average(before, read_counter());
    }
}

#[cold]
fn calibrate() -> f64 {
    #[cfg(target_arch = "aarch64")]
    {
        let frequency: u64;
        // The architectural counter frequency is constant across frequency scaling.
        unsafe { core::arch::asm!("mrs {}, cntfrq_el0", out(reg) frequency) };
        return 1_000_000_000.0 / frequency as f64;
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let mut rates = [0.0; CALIBRATION_TRIALS];
        for rate in &mut rates {
            let start_time = Instant::now();
            let start_counter = read_counter();
            let elapsed = loop {
                let elapsed = start_time.elapsed().as_nanos() as u64;
                if elapsed >= CALIBRATION_INTERVAL_NS {
                    break elapsed;
                }
                std::hint::spin_loop();
            };
            *rate = read_counter().wrapping_sub(start_counter) as f64 / elapsed as f64;
        }
        rates.sort_unstable_by(f64::total_cmp);
        return 1.0 / rates[CALIBRATION_TRIALS / 2];
    }

    #[allow(unreachable_code)]
    1.0
}

#[inline(always)]
fn read_counter() -> u64 {
    #[cfg(target_arch = "x86")]
    unsafe {
        return core::arch::x86::_rdtsc();
    }
    #[cfg(target_arch = "x86_64")]
    unsafe {
        return core::arch::x86_64::_rdtsc();
    }
    #[cfg(target_arch = "aarch64")]
    {
        let value: u64;
        unsafe { core::arch::asm!("mrs {}, cntvct_el0", out(reg) value) };
        return value;
    }
    #[allow(unreachable_code)]
    quent_time::timestamp()
}

const fn fast_average(left: u64, right: u64) -> u64 {
    (left & right) + ((left ^ right) >> 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converted_counter_tracks_timestamp() {
        let mut converter = TimestampConverter::new();
        let converted = converter.convert(capture());
        let system = quent_time::timestamp();
        assert!(system.abs_diff(converted) < 5_000_000);
    }

    #[test]
    fn converted_counter_is_monotonic() {
        let mut converter = TimestampConverter::new();
        let first = converter.convert(capture());
        let second = converter.convert(capture());
        assert!(second >= first);
    }
}
