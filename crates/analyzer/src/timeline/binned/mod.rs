// SPDX-FileCopyrightText: Copyright (c) 2026, NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Functionality to analyze telemetry to construct timelines with time bins.

use rustc_hash::FxHashMap as HashMap;

use quent_time::{SpanNanoSec, TimeNanoSec, bin::BinnedSpan};

use crate::AnalyzerResult;

pub mod categorical;
pub mod resource;

/// A trait for types that can aggregate items into a sequence of time bins.
pub(crate) trait BinnedTimelineAggregator {
    type Item;
    type Output;

    /// Return the configuration of the binned timeline.
    fn config(&self) -> BinnedSpan;

    /// Attempt to push an item into all bins that intersect with the given time
    /// span.
    ///
    /// # Arguments
    ///
    /// * `span` - The time span that determines which bins should receive the
    ///   item.
    /// * `item` - The item to be pushed into all intersecting bins.
    fn try_push(&mut self, span: SpanNanoSec, item: Self::Item) -> AnalyzerResult<()>;

    /// Attempt to return the finished output of this aggregator.
    fn finish(self) -> Self::Output;
}

/// How values are aggregated within each time bin.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum AggregationMode {
    /// Sum values weighted by overlap fraction (for occupancy resources).
    #[default]
    Sum,
    /// Sweep-line peak aggregate rate per bin (for rate resources).
    /// Buffers all (span, rate) pairs, then at finish time computes
    /// the peak sum-of-concurrent-rates within each bin.
    Rate,
}

/// A binned timeline built from numeric primitive values associated with a
/// span.
pub(crate) struct UnitAggregator {
    config: BinnedSpan,
    bins: Vec<f64>,
    mode: AggregationMode,
    /// Buffered (start, end, rate) tuples for sweep-line computation in Rate mode.
    rate_events: Vec<(TimeNanoSec, TimeNanoSec, f64)>,
}

impl UnitAggregator {
    pub(crate) fn with_mode(config: BinnedSpan, mode: AggregationMode) -> Self {
        let capacity = config.num_bins().get() as usize;
        Self {
            config,
            bins: std::iter::repeat_with(Default::default)
                .take(capacity)
                .collect(),
            mode,
            rate_events: Vec::new(),
        }
    }

    /// Sweep-line computation: find the peak aggregate rate within each bin.
    ///
    /// 1. Create +rate events at each span start and -rate events at each span
    ///    end.
    /// 2. Sort by timestamp (ties broken: ends before starts so we don't
    ///    overcount at boundaries).
    /// 3. Sweep through, maintaining a running sum of active rates.
    /// 4. Between consecutive event timestamps, the aggregate rate is constant.
    ///    For every bin that overlaps with that interval, update the bin's peak.
    fn sweep_line_finish(&mut self) {
        if self.rate_events.is_empty() {
            return;
        }

        // Clamp all events to the binned span's range.
        let span_start = self.config.span.start();
        let span_end = self.config.span.end();

        // Build delta events: (timestamp, delta_rate).
        // Use +rate at start, -rate at end.
        let mut deltas: Vec<(TimeNanoSec, f64)> = Vec::with_capacity(self.rate_events.len() * 2);
        for &(start, end, rate) in &self.rate_events {
            let clamped_start = start.max(span_start);
            let clamped_end = end.min(span_end);
            if clamped_start >= clamped_end {
                continue;
            }
            deltas.push((clamped_start, rate));
            deltas.push((clamped_end, -rate));
        }

        if deltas.is_empty() {
            return;
        }

        // Sort: by timestamp, then ends (-rate) before starts (+rate) at same
        // timestamp so the running sum doesn't transiently overcount.
        deltas.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap()));

        // Sweep through events, tracking the running aggregate rate.
        let mut current_rate: f64 = 0.0;
        let mut prev_time = deltas[0].0;

        for &(time, delta) in &deltas {
            // The interval [prev_time, time) had rate = current_rate.
            if time > prev_time && current_rate > 0.0 {
                // Find all bins overlapping [prev_time, time) and update peaks.
                if let Ok(interval) = SpanNanoSec::try_new(prev_time, time) {
                    for (index, _) in self.config.iter_indices_intersect_durations(&interval) {
                        let bin = &mut self.bins[index as usize];
                        if current_rate > *bin {
                            *bin = current_rate;
                        }
                    }
                }
            }
            current_rate += delta;
            prev_time = time;
        }

        // Handle floating point drift — clamp to zero.
        if current_rate.abs() < 1e-10 {
            let _ = current_rate;
        }
    }
}

impl BinnedTimelineAggregator for UnitAggregator {
    type Item = f64;
    type Output = Vec<f64>;

    fn config(&self) -> BinnedSpan {
        self.config
    }

    fn try_push(&mut self, span: SpanNanoSec, item: Self::Item) -> AnalyzerResult<()> {
        let span_duration = span.duration();
        if span_duration == 0 {
            return Ok(());
        }
        match self.mode {
            AggregationMode::Sum => {
                for (index, intersect_duration) in
                    self.config().iter_indices_intersect_durations(&span)
                {
                    let overlap_fraction =
                        intersect_duration as f64 / self.config().bin_duration().get() as f64;
                    assert!(overlap_fraction >= 0.0);
                    assert!(overlap_fraction <= 1.0);
                    self.bins[index as usize] += overlap_fraction * item
                }
            }
            AggregationMode::Rate => {
                self.rate_events.push((span.start(), span.end(), item));
            }
        }

        Ok(())
    }

    fn finish(mut self) -> Self::Output {
        if self.mode == AggregationMode::Rate {
            self.sweep_line_finish();
        }
        self.bins
    }
}

/// A binned timeline built from numeric primitive values associated with a
/// span and a name.
pub(crate) struct KeyedAggregator<Key> {
    config: BinnedSpan,
    mode: AggregationMode,
    bins: HashMap<Key, UnitAggregator>,
}

impl<Key> KeyedAggregator<Key> {
    pub(crate) fn new(config: BinnedSpan) -> Self {
        Self::with_mode(config, AggregationMode::Sum)
    }

    pub(crate) fn with_mode(config: BinnedSpan, mode: AggregationMode) -> Self {
        Self {
            config,
            mode,
            bins: HashMap::default(),
        }
    }
}

impl<Key> BinnedTimelineAggregator for KeyedAggregator<Key>
where
    Key: Eq + std::hash::Hash,
{
    type Item = (Key, f64);
    type Output = HashMap<Key, Vec<f64>>;

    fn config(&self) -> BinnedSpan {
        self.config
    }

    fn try_push(&mut self, span: SpanNanoSec, item: Self::Item) -> AnalyzerResult<()> {
        self.bins
            .entry(item.0)
            .or_insert_with(|| UnitAggregator::with_mode(self.config, self.mode))
            .try_push(span, item.1)
    }

    fn finish(self) -> Self::Output {
        self.bins
            .into_iter()
            .map(|(k, v)| (k, v.finish()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZero;

    #[test]
    fn unit_aggregator() -> AnalyzerResult<()> {
        let config = BinnedSpan::try_new(
            SpanNanoSec::try_new(0, 80).unwrap(),
            NonZero::new(4).unwrap(),
        )
        .unwrap();

        let mut aggregator = UnitAggregator::with_mode(config, AggregationMode::Sum);

        aggregator.try_push(SpanNanoSec::try_new(0, 30).unwrap(), 10.0)?;
        aggregator.try_push(SpanNanoSec::try_new(20, 60).unwrap(), 10.0)?;

        assert_eq!(aggregator.finish(), [10.0, 15.0, 10.0, 0.0]);

        Ok(())
    }

    fn ten_bin_config() -> BinnedSpan {
        BinnedSpan::try_new(
            SpanNanoSec::try_new(0, 1000).unwrap(),
            NonZero::new(10).unwrap(),
        )
        .unwrap()
    }

    /// Overlapping spans accumulate span-weighted fractions per bin.
    #[test]
    fn keyed_aggregator_span_weighting_across_bin_boundaries() -> AnalyzerResult<()> {
        let mut aggregator: KeyedAggregator<&str> = KeyedAggregator::new(ten_bin_config());

        aggregator.try_push(SpanNanoSec::try_new(0, 300).unwrap(), ("k", 1.0))?;
        aggregator.try_push(SpanNanoSec::try_new(250, 450).unwrap(), ("k", 1.0))?;

        let bins = aggregator.finish();
        assert_eq!(
            bins.get("k").unwrap()[..],
            [1.0, 1.0, 1.5, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        Ok(())
    }

    /// Zero-duration spans create the key but contribute nothing.
    #[test]
    fn keyed_aggregator_zero_duration_span_is_noop() -> AnalyzerResult<()> {
        let mut aggregator: KeyedAggregator<&str> = KeyedAggregator::new(ten_bin_config());
        aggregator.try_push(SpanNanoSec::try_new(500, 500).unwrap(), ("k", 1.0))?;

        let bins = aggregator.finish();
        assert_eq!(bins.get("k").unwrap()[..], [0.0; 10]);
        Ok(())
    }

    /// Spans entirely outside the window contribute nothing.
    #[test]
    fn keyed_aggregator_out_of_window_span_contributes_nothing() -> AnalyzerResult<()> {
        let mut aggregator: KeyedAggregator<&str> = KeyedAggregator::new(ten_bin_config());
        aggregator.try_push(SpanNanoSec::try_new(2000, 3000).unwrap(), ("k", 1.0))?;

        let bins = aggregator.finish();
        assert_eq!(bins.get("k").unwrap()[..], [0.0; 10]);
        Ok(())
    }

    #[test]
    fn rate_aggregator_peak() -> AnalyzerResult<()> {
        // 4 bins of 20ns each: [0,20), [20,40), [40,60), [60,80)
        let config = BinnedSpan::try_new(
            SpanNanoSec::try_new(0, 80).unwrap(),
            NonZero::new(4).unwrap(),
        )
        .unwrap();

        let mut aggregator = UnitAggregator::with_mode(config, AggregationMode::Rate);

        // Transfer A: rate 5.0 during [0, 40)
        aggregator.try_push(SpanNanoSec::try_new(0, 40).unwrap(), 5.0)?;
        // Transfer B: rate 3.0 during [20, 60)
        aggregator.try_push(SpanNanoSec::try_new(20, 60).unwrap(), 3.0)?;

        let result = aggregator.finish();
        // Bin 0 [0,20):  only A active → peak = 5.0
        // Bin 1 [20,40): A + B active  → peak = 8.0
        // Bin 2 [40,60): only B active → peak = 3.0
        // Bin 3 [60,80): nothing       → peak = 0.0
        assert_eq!(result, [5.0, 8.0, 3.0, 0.0]);

        Ok(())
    }

    #[test]
    fn rate_aggregator_short_burst() -> AnalyzerResult<()> {
        // Verify a short burst that partially overlaps a bin shows full rate.
        // 4 bins of 20ns each: [0,20), [20,40), [40,60), [60,80)
        let config = BinnedSpan::try_new(
            SpanNanoSec::try_new(0, 80).unwrap(),
            NonZero::new(4).unwrap(),
        )
        .unwrap();

        let mut aggregator = UnitAggregator::with_mode(config, AggregationMode::Rate);

        // Short burst: rate 10.0 during [15, 25) — overlaps bins 0 and 1
        aggregator.try_push(SpanNanoSec::try_new(15, 25).unwrap(), 10.0)?;

        let result = aggregator.finish();
        // Both bins see the full rate, not a diluted fraction.
        assert_eq!(result, [10.0, 10.0, 0.0, 0.0]);

        Ok(())
    }
}
