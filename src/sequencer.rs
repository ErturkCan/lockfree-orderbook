//! Event sequencer for assigning monotonic sequence numbers.
//!
//! The sequencer is the single point of ordering in the system. All external
//! events (order submissions, cancellations) pass through here to be assigned
//! a global sequence number and timestamp. This enables:
//!
//! 1. Deterministic replay (via the journal)
//! 2. FIFO ordering across multiple producers (future lock-free design)
//! 3. Precise time-priority ordering within the matching engine
//!
//! Current implementation is single-threaded; can be extended with
//! crossbeam channels for multi-producer scenarios.

use crate::order::OrderEvent;
use std::time::Instant;

/// Event sequencer assigning monotonic sequence numbers and timestamps.
///
/// In production, this would be fed by a lock-free MPMC queue from
/// multiple order sources. The sequencer then publishes sequenced events
/// to the matching engine and journal.
#[derive(Debug)]
pub struct Sequencer {
    /// Next sequence number to assign
    next_sequence: u64,

    /// Engine start time for timestamp calculation
    start_time: Instant,
}

/// A sequenced order event with assigned sequence number and timestamp
#[derive(Debug, Clone)]
pub struct SequencedEvent {
    /// Global sequence number
    pub sequence: u64,

    /// Time since sequencer creation (nanoseconds)
    pub timestamp_ns: u64,

    /// The actual order event
    pub event: OrderEvent,
}

impl Sequencer {
    /// Create a new sequencer
    pub fn new() -> Self {
        Sequencer {
            next_sequence: 1,
            start_time: Instant::now(),
        }
    }

    /// Sequence an incoming order event
    ///
    /// Assigns the next monotonic sequence number and captures current timestamp.
    /// This is the single ordering point for all external events.
    pub fn sequence(&mut self, event: OrderEvent) -> SequencedEvent {
        let sequence = self.next_sequence;
        self.next_sequence += 1;

        let elapsed = self.start_time.elapsed();
        let timestamp_ns = elapsed.as_secs() * 1_000_000_000 + elapsed.subsec_nanos() as u64;

        SequencedEvent {
            sequence,
            timestamp_ns,
            event,
        }
    }

    /// Get the next sequence number (without consuming)
    pub fn peek_sequence(&self) -> u64 {
        self.next_sequence
    }

    /// Get total events sequenced
    pub fn total_sequenced(&self) -> u64 {
        self.next_sequence - 1
    }
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{Price, Quantity, Side};

    #[test]
    fn test_monotonic_sequence() {
        let mut sequencer = Sequencer::new();

        let event1 = sequencer.sequence(OrderEvent::Limit {
            side: Side::Bid,
            price: Price(10000000000),
            quantity: Quantity(100),
        });

        let event2 = sequencer.sequence(OrderEvent::Limit {
            side: Side::Ask,
            price: Price(10000000000),
            quantity: Quantity(100),
        });

        assert_eq!(event1.sequence, 1);
        assert_eq!(event2.sequence, 2);
        assert!(event2.timestamp_ns >= event1.timestamp_ns);
    }

    #[test]
    fn test_sequencer_total() {
        let mut sequencer = Sequencer::new();

        for _ in 0..10 {
            sequencer.sequence(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(10000000000),
                quantity: Quantity(100),
            });
        }

        assert_eq!(sequencer.total_sequenced(), 10);
        assert_eq!(sequencer.peek_sequence(), 11);
    }
}
