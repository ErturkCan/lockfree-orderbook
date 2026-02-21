//! Order types and event definitions.
//!
//! This module defines the core order representation and event types used
//! throughout the matching engine. Orders use arena-style IDs assigned by
//! the sequencer to maintain allocation efficiency.

use std::fmt;

/// Unique order identifier assigned by the sequencer.
/// Monotonically increasing, enabling efficient array-based order storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrderId(pub u64);

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Side of the order: bid (buy) or ask (sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    /// Buy order
    Bid,
    /// Sell order
    Ask,
}

impl Side {
    /// Get the opposite side
    pub fn opposite(self) -> Side {
        match self {
            Side::Bid => Side::Ask,
            Side::Ask => Side::Bid,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Bid => write!(f, "BID"),
            Side::Ask => write!(f, "ASK"),
        }
    }
}

/// Fixed-point price representation (10^-8 precision).
/// Uses u64 to represent price * 10^8, enabling integer-only arithmetic.
/// Range: 0.00000001 to 1844.67 (for u64::MAX / 10^8)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(pub u64);

impl Price {
    /// Create a price from decimal representation (e.g., 100.5 -> Price(10050000000))
    pub fn from_decimal(value: f64) -> Self {
        Price((value * 1e8) as u64)
    }

    /// Convert price to decimal for display
    pub fn to_decimal(self) -> f64 {
        self.0 as f64 / 1e8
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.8}", self.to_decimal())
    }
}

/// Order quantity in base units (shares, contracts, satoshis, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quantity(pub u64);

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Core order representation.
///
/// Immutable after creation. Orders are identified by OrderId assigned
/// during sequencing. The timestamp is captured at sequencing time for
/// consistent ordering across multiple execution paths.
#[derive(Debug, Clone)]
pub struct Order {
    /// Unique order identifier (assigned by sequencer)
    pub id: OrderId,
    /// Order side (bid or ask)
    pub side: Side,
    /// Limit price
    pub price: Price,
    /// Order quantity
    pub quantity: Quantity,
    /// Remaining quantity available for matching
    pub remaining: Quantity,
    /// Sequence number for time priority
    pub sequence: u64,
    /// Timestamp (nanoseconds since engine start)
    pub timestamp_ns: u64,
}

impl Order {
    /// Create a new order with the given parameters.
    pub fn new(
        id: OrderId,
        side: Side,
        price: Price,
        quantity: Quantity,
        sequence: u64,
        timestamp_ns: u64,
    ) -> Self {
        Order {
            id,
            side,
            price,
            quantity,
            remaining: quantity,
            sequence,
            timestamp_ns,
        }
    }

    /// Check if order is fully filled
    pub fn is_filled(&self) -> bool {
        self.remaining.0 == 0
    }

    /// Check if order still has remaining quantity
    pub fn is_live(&self) -> bool {
        self.remaining.0 > 0
    }

    /// Reduce remaining quantity by the given amount
    pub fn reduce_remaining(&mut self, amount: Quantity) {
        self.remaining.0 = self.remaining.0.saturating_sub(amount.0);
    }
}

/// Type alias for order updates used in matching
#[derive(Debug, Clone, Copy)]
pub struct Fill {
    pub maker_id: OrderId,
    pub taker_id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
}

/// External order event submitted to the sequencer.
///
/// These are the events that clients submit to the matching engine.
/// The sequencer will assign a sequence number and timestamp to each event.
#[derive(Debug, Clone)]
pub enum OrderEvent {
    /// New limit order
    Limit {
        side: Side,
        price: Price,
        quantity: Quantity,
    },
    /// Market order (fill immediately at best available price)
    Market {
        side: Side,
        quantity: Quantity,
    },
    /// Cancel an existing order by its ID
    Cancel {
        order_id: OrderId,
    },
    /// Modify an existing order (price and/or quantity)
    Modify {
        order_id: OrderId,
        new_price: Option<Price>,
        new_quantity: Option<Quantity>,
    },
}

impl fmt::Display for OrderEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderEvent::Limit { side, price, quantity } => {
                write!(f, "Limit({} {} @ {})", side, quantity, price)
            }
            OrderEvent::Market { side, quantity } => {
                write!(f, "Market({} {})", side, quantity)
            }
            OrderEvent::Cancel { order_id } => {
                write!(f, "Cancel({})", order_id)
            }
            OrderEvent::Modify { order_id, new_price, new_quantity } => {
                write!(
                    f,
                    "Modify({}, price={:?}, qty={:?})",
                    order_id, new_price, new_quantity
                )
            }
        }
    }
}
