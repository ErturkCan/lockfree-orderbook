//! Core matching engine implementing price-time priority matching.
//!
//! The engine processes sequenced order events and produces fills. It maintains
//! the order book state and executes the matching algorithm. This is where
//! the critical trading logic resides.
//!
//! Key design points:
//! - Single-threaded sequencer pattern: All ordering happens in the sequencer,
//!   matching happens in the engine. This separates concerns and enables
//!   lock-free designs in production.
//! - No allocation in matching path: Uses pre-allocated book and event handling.
//! - Price-time priority: Bids sorted highest-first (via BTreeMap Reverse),
//!   asks sorted lowest-first. Within each level, FIFO matching.

use crate::book::OrderBook;
use crate::journal::EventJournal;
use crate::metrics::Metrics;
use crate::order::{Order, OrderEvent, OrderId, Price, Quantity, Side};
use crate::sequencer::{Sequencer, SequencedEvent};
use std::time::Instant;
use thiserror::Error;

/// Matching engine errors
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Order not found: {0}")]
    OrderNotFound(OrderId),

    #[error("Invalid order: {0}")]
    InvalidOrder(String),
}

/// Result type for engine operations
pub type Result<T> = std::result::Result<T, EngineError>;

/// The matching engine processes order events and maintains the order book.
///
/// The engine is the state machine of the trading system. Orders flow through
/// the sequencer, into the engine, which updates the book and produces fills.
///
/// Architecture:
/// ```text
/// External Orders
///     |
///     v
/// Sequencer (assigns sequence #, timestamp)
///     |
///     v
/// MatchingEngine (maintains book, matches orders)
///     |
///     +---> Journal (records for audit/replay)
///     +---> Fills (immediate output)
/// ```
pub struct MatchingEngine {
    /// The order book
    book: OrderBook,

    /// Sequencer for assigning ordering and timestamps
    sequencer: Sequencer,

    /// Event journal for audit and replay
    journal: EventJournal,

    /// Performance metrics
    metrics: Metrics,
}

/// Output from processing an order: fills generated
#[derive(Debug, Clone)]
pub struct OrderResponse {
    /// Sequence number assigned by sequencer
    pub sequence: u64,

    /// Fills that were generated
    pub fills: Vec<crate::order::Fill>,

    /// Remaining order if not fully filled
    pub remaining: Option<Order>,
}

impl MatchingEngine {
    /// Create a new matching engine
    pub fn new() -> Self {
        MatchingEngine {
            book: OrderBook::new(),
            sequencer: Sequencer::new(),
            journal: EventJournal::new(),
            metrics: Metrics::new(),
        }
    }

    /// Create with pre-allocated capacity
    pub fn with_capacity(journal_capacity: usize) -> Self {
        MatchingEngine {
            book: OrderBook::new(),
            sequencer: Sequencer::new(),
            journal: EventJournal::with_capacity(journal_capacity),
            metrics: Metrics::new(),
        }
    }

    /// Process an incoming order event
    ///
    /// This is the main entry point for external events. The engine:
    /// 1. Sequences the event (assigns seq #, timestamp)
    /// 2. Converts event to order
    /// 3. Processes through the book
    /// 4. Records in journal
    /// 5. Updates metrics
    pub fn process_event(&mut self, event: OrderEvent) -> Result<OrderResponse> {
        let t0 = Instant::now();

        // Sequence the event
        let sequenced = self.sequencer.sequence(event.clone());
        let sequence = sequenced.sequence;
        let timestamp_ns = sequenced.timestamp_ns;

        // Record in journal
        self.journal.append_event(sequenced);

        // Process based on event type
        let (remaining, fills) = match event {
            OrderEvent::Limit {
                side,
                price,
                quantity,
            } => {
                let order_id = OrderId(sequence);
                let order = Order::new(order_id, side, price, quantity, sequence, timestamp_ns);

                let (remaining, fills) = self.book.add_limit_order(order);

                // Record fills in journal
                for fill in &fills {
                    self.journal.append_fill(*fill);
                }

                (remaining, fills)
            }

            OrderEvent::Market { side, quantity } => {
                // Market orders match immediately at best available prices
                // Implementation: convert to limit order at extreme price
                let extreme_price = match side {
                    Side::Bid => Price(u64::MAX), // Will match any ask
                    Side::Ask => Price(0),         // Will match any bid
                };

                let order_id = OrderId(sequence);
                let order =
                    Order::new(order_id, side, extreme_price, quantity, sequence, timestamp_ns);

                let (remaining, fills) = self.book.add_limit_order(order);

                for fill in &fills {
                    self.journal.append_fill(*fill);
                }

                (remaining, fills)
            }

            OrderEvent::Cancel { order_id } => {
                if let Some(_cancelled) = self.book.cancel_order(order_id) {
                    (None, vec![])
                } else {
                    return Err(EngineError::OrderNotFound(order_id));
                }
            }

            OrderEvent::Modify {
                order_id,
                new_price,
                new_quantity,
            } => {
                if self.book.modify_order(order_id, new_price, new_quantity) {
                    (None, vec![])
                } else {
                    return Err(EngineError::OrderNotFound(order_id));
                }
            }
        };

        // Update metrics
        let latency_ns = t0.elapsed().as_nanos() as u64;
        self.metrics.inc_order_count();
        self.metrics.record_order_latency(latency_ns);
        self.metrics.add_fill_count(fills.len());
        self.metrics
            .set_orders_in_book((self.book.total_bid_quantity().0 + self.book.total_ask_quantity().0));
        self.metrics.set_active_bids(self.book.bid_levels() as u64);
        self.metrics.set_active_asks(self.book.ask_levels() as u64);

        Ok(OrderResponse {
            sequence,
            fills,
            remaining,
        })
    }

    /// Get the best bid price
    pub fn best_bid(&self) -> Option<Price> {
        self.book.best_bid()
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<Price> {
        self.book.best_ask()
    }

    /// Get current spread
    pub fn spread(&self) -> Option<i64> {
        self.book.spread()
    }

    /// Get an order from the book
    pub fn get_order(&self, order_id: OrderId) -> Option<Order> {
        self.book.get_order(order_id)
    }

    /// Get metrics
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Get mutable metrics
    pub fn metrics_mut(&mut self) -> &mut Metrics {
        &mut self.metrics
    }

    /// Get journal reference
    pub fn journal(&self) -> &EventJournal {
        &self.journal
    }

    /// Get sequencer reference
    pub fn sequencer(&self) -> &Sequencer {
        &self.sequencer
    }

    /// Get total orders in book
    pub fn total_orders_in_book(&self) -> u64 {
        self.book.total_bid_quantity().0 + self.book.total_ask_quantity().0
    }

    /// Get bid side quantity
    pub fn bid_quantity(&self) -> Quantity {
        self.book.total_bid_quantity()
    }

    /// Get ask side quantity
    pub fn ask_quantity(&self) -> Quantity {
        self.book.total_ask_quantity()
    }
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limit_order() {
        let mut engine = MatchingEngine::new();

        let response = engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(10000000000),
                quantity: Quantity(100),
            })
            .unwrap();

        assert_eq!(response.sequence, 1);
        assert_eq!(response.fills.len(), 0);
        assert!(response.remaining.is_some());
    }

    #[test]
    fn test_matching() {
        let mut engine = MatchingEngine::new();

        let _bid_response = engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(10000000000),
                quantity: Quantity(100),
            })
            .unwrap();

        let ask_response = engine
            .process_event(OrderEvent::Limit {
                side: Side::Ask,
                price: Price(10000000000),
                quantity: Quantity(100),
            })
            .unwrap();

        assert_eq!(ask_response.fills.len(), 1);
        assert!(ask_response.remaining.is_none());
    }

    #[test]
    fn test_cancel_order() {
        let mut engine = MatchingEngine::new();

        let bid_response = engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(10000000000),
                quantity: Quantity(100),
            })
            .unwrap();

        let order_id = OrderId(bid_response.sequence);

        let cancel_result = engine.process_event(OrderEvent::Cancel { order_id });
        assert!(cancel_result.is_ok());

        // Trying to cancel again should fail
        let cancel_again = engine.process_event(OrderEvent::Cancel { order_id });
        assert!(cancel_again.is_err());
    }

    #[test]
    fn test_market_order() {
        let mut engine = MatchingEngine::new();

        let _bid = engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(10000000000),
                quantity: Quantity(100),
            })
            .unwrap();

        let market = engine
            .process_event(OrderEvent::Market {
                side: Side::Ask,
                quantity: Quantity(100),
            })
            .unwrap();

        assert_eq!(market.fills.len(), 1);
        assert_eq!(market.fills[0].quantity.0, 100);
    }

    #[test]
    fn test_metrics_update() {
        let mut engine = MatchingEngine::new();

        engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(10000000000),
                quantity: Quantity(100),
            })
            .unwrap();

        assert_eq!(engine.metrics().total_orders(), 1);
        assert_eq!(engine.metrics().total_fills(), 0);
    }
}
