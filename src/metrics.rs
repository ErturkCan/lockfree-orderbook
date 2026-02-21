//! Low-latency metrics tracking with zero allocations in hot paths.
//!
//! This module tracks engine performance metrics with allocation-free measurement.
//! All measurements use std::time::Instant, avoiding any syscalls or heap allocation
//! in the critical matching path.
//!
//! Metrics are updated directly during order processing and can be queried at any time
//! for monitoring and diagnostics.

use std::time::Instant;

/// Statistics for latency measurements
#[derive(Debug, Clone, Copy)]
pub struct LatencyStats {
    /// Minimum latency (nanoseconds)
    pub min_ns: u64,
    /// Maximum latency (nanoseconds)
    pub max_ns: u64,
    /// Total accumulated latency (nanoseconds)
    pub total_ns: u64,
    /// Number of measurements
    pub count: u64,
}

impl LatencyStats {
    /// Calculate average latency
    pub fn avg_ns(&self) -> f64 {
        if self.count > 0 {
            self.total_ns as f64 / self.count as f64
        } else {
            0.0
        }
    }

    /// Create empty stats
    fn new() -> Self {
        LatencyStats {
            min_ns: u64::MAX,
            max_ns: 0,
            total_ns: 0,
            count: 0,
        }
    }

    /// Update stats with a new measurement
    fn record(&mut self, latency_ns: u64) {
        self.min_ns = self.min_ns.min(latency_ns);
        self.max_ns = self.max_ns.max(latency_ns);
        self.total_ns += latency_ns;
        self.count += 1;
    }
}

/// Engine-wide metrics collector.
///
/// All recording happens via mutation, so the hot path is:
/// - Capture time: `let t0 = Instant::now();`
/// - Do work
/// - Record: `metrics.record_order_latency(t0.elapsed().as_nanos() as u64);`
///
/// This incurs only one Instant capture and one arithmetic operation
/// in the critical path.
pub struct Metrics {
    /// Order insertion latencies
    order_latency: LatencyStats,

    /// Matching (fill generation) latencies
    match_latency: LatencyStats,

    /// Total orders processed
    total_orders: u64,

    /// Total fills generated
    total_fills: u64,

    /// Orders currently in the book
    orders_in_book: u64,

    /// Number of active bids
    active_bids: u64,

    /// Number of active asks
    active_asks: u64,
}

impl Metrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        Metrics {
            order_latency: LatencyStats::new(),
            match_latency: LatencyStats::new(),
            total_orders: 0,
            total_fills: 0,
            orders_in_book: 0,
            active_bids: 0,
            active_asks: 0,
        }
    }

    /// Record order insertion latency
    pub fn record_order_latency(&mut self, latency_ns: u64) {
        self.order_latency.record(latency_ns);
    }

    /// Record matching latency
    pub fn record_match_latency(&mut self, latency_ns: u64) {
        self.match_latency.record(latency_ns);
    }

    /// Increment order count
    pub fn inc_order_count(&mut self) {
        self.total_orders += 1;
    }

    /// Increment fill count
    pub fn add_fill_count(&mut self, count: usize) {
        self.total_fills += count as u64;
    }

    /// Update orders in book
    pub fn set_orders_in_book(&mut self, count: u64) {
        self.orders_in_book = count;
    }

    /// Update active bids
    pub fn set_active_bids(&mut self, count: u64) {
        self.active_bids = count;
    }

    /// Update active asks
    pub fn set_active_asks(&mut self, count: u64) {
        self.active_asks = count;
    }

    // Getters

    /// Get order latency stats
    pub fn order_latency(&self) -> LatencyStats {
        self.order_latency
    }

    /// Get match latency stats
    pub fn match_latency(&self) -> LatencyStats {
        self.match_latency
    }

    /// Get total orders processed
    pub fn total_orders(&self) -> u64 {
        self.total_orders
    }

    /// Get total fills
    pub fn total_fills(&self) -> u64 {
        self.total_fills
    }

    /// Get orders currently in book
    pub fn orders_in_book(&self) -> u64 {
        self.orders_in_book
    }

    /// Get active bid price levels
    pub fn active_bids(&self) -> u64 {
        self.active_bids
    }

    /// Get active ask price levels
    pub fn active_asks(&self) -> u64 {
        self.active_asks
    }

    /// Get average fill ratio (fills per order)
    pub fn avg_fills_per_order(&self) -> f64 {
        if self.total_orders > 0 {
            self.total_fills as f64 / self.total_orders as f64
        } else {
            0.0
        }
    }

    /// Reset all metrics
    pub fn reset(&mut self) {
        self.order_latency = LatencyStats::new();
        self.match_latency = LatencyStats::new();
        self.total_orders = 0;
        self.total_fills = 0;
        self.orders_in_book = 0;
        self.active_bids = 0;
        self.active_asks = 0;
    }

    /// Format metrics as a human-readable report
    pub fn report(&self) -> String {
        format!(
            "=== Order Matching Engine Metrics ===\n\
             Total Orders: {}\n\
             Total Fills: {}\n\
             Avg Fills/Order: {:.2}\n\
             Orders in Book: {}\n\
             Active Bid Levels: {}\n\
             Active Ask Levels: {}\n\
             \n\
             Order Insertion Latency:\n\
               Min: {:.3} µs\n\
               Max: {:.3} µs\n\
               Avg: {:.3} µs\n\
             \n\
             Matching Latency:\n\
               Min: {:.3} µs\n\
               Max: {:.3} µs\n\
               Avg: {:.3} µs\n",
            self.total_orders,
            self.total_fills,
            self.avg_fills_per_order(),
            self.orders_in_book,
            self.active_bids,
            self.active_asks,
            self.order_latency.min_ns as f64 / 1000.0,
            self.order_latency.max_ns as f64 / 1000.0,
            self.order_latency.avg_ns() / 1000.0,
            self.match_latency.min_ns as f64 / 1000.0,
            self.match_latency.max_ns as f64 / 1000.0,
            self.match_latency.avg_ns() / 1000.0,
        )
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_stats() {
        let mut stats = LatencyStats::new();
        stats.record(100);
        stats.record(200);
        stats.record(150);

        assert_eq!(stats.min_ns, 100);
        assert_eq!(stats.max_ns, 200);
        assert_eq!(stats.count, 3);
        assert_eq!(stats.total_ns, 450);
        assert!(f64::abs(stats.avg_ns() - 150.0) < 0.01);
    }

    #[test]
    fn test_metrics_tracking() {
        let mut metrics = Metrics::new();

        metrics.inc_order_count();
        metrics.record_order_latency(1000);
        metrics.add_fill_count(1);

        assert_eq!(metrics.total_orders(), 1);
        assert_eq!(metrics.total_fills(), 1);
        assert_eq!(metrics.order_latency().count, 1);
    }
}
