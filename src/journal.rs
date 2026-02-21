//! Append-only event journal for replay and auditability.
//!
//! The journal records all sequenced events and fills, enabling:
//! - Deterministic replay of the order book state
//! - Audit trail of all matching events
//! - Recovery from snapshots
//!
//! Current implementation uses a simple Vec for fast append operations.
//! In production, this would be backed by a memory-mapped file or
//! distributed log for durability and horizontal scaling.

use crate::order::Fill;
use crate::sequencer::SequencedEvent;

/// Journal entry representing either an order event or a fill
#[derive(Debug, Clone)]
pub enum JournalEntry {
    /// Sequenced order event
    OrderEvent(SequencedEvent),
    /// Fill event
    Fill(Fill),
}

/// Append-only event journal
///
/// All events that pass through the system are logged here in order.
/// This enables deterministic replay: replaying these events through
/// an empty order book produces identical state.
#[derive(Debug)]
pub struct EventJournal {
    /// All journal entries in order
    entries: Vec<JournalEntry>,
}

impl EventJournal {
    /// Create a new empty journal
    pub fn new() -> Self {
        EventJournal {
            entries: Vec::new(),
        }
    }

    /// Create a journal with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        EventJournal {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// Append an order event to the journal
    pub fn append_event(&mut self, event: SequencedEvent) {
        self.entries.push(JournalEntry::OrderEvent(event));
    }

    /// Append a fill to the journal
    pub fn append_fill(&mut self, fill: Fill) {
        self.entries.push(JournalEntry::Fill(fill));
    }

    /// Get all journal entries
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if journal is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get total number of order events
    pub fn order_event_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, JournalEntry::OrderEvent(_)))
            .count()
    }

    /// Get total number of fills
    pub fn fill_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| matches!(e, JournalEntry::Fill(_)))
            .count()
    }

    /// Clear the journal
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for EventJournal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{OrderEvent, Price, Quantity, Side};

    #[test]
    fn test_append_and_retrieve() {
        let mut journal = EventJournal::new();

        let event = SequencedEvent {
            sequence: 1,
            timestamp_ns: 0,
            event: OrderEvent::Limit {
                side: Side::Bid,
                price: Price(10000000000),
                quantity: Quantity(100),
            },
        };

        journal.append_event(event.clone());
        assert_eq!(journal.len(), 1);
        assert_eq!(journal.order_event_count(), 1);

        if let JournalEntry::OrderEvent(stored) = &journal.entries()[0] {
            assert_eq!(stored.sequence, 1);
        } else {
            panic!("Expected OrderEvent");
        }
    }

    #[test]
    fn test_append_fills() {
        use crate::order::OrderId;

        let mut journal = EventJournal::new();

        let fill = Fill {
            maker_id: OrderId(1),
            taker_id: OrderId(2),
            price: Price(10000000000),
            quantity: Quantity(50),
        };

        journal.append_fill(fill);
        assert_eq!(journal.fill_count(), 1);
        assert_eq!(journal.len(), 1);
    }

    #[test]
    fn test_journal_capacity() {
        let journal = EventJournal::with_capacity(1000);
        assert_eq!(journal.len(), 0);
    }
}
