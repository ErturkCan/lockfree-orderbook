//! Order book implementation using BTreeMap for efficient price level access.
//!
//! The order book maintains separate bid and ask sides, each as a BTreeMap
//! mapping price levels to order queues. Bids are stored in reverse order
//! (highest price first) for efficient best-price access.
//!
//! Design: We use BTreeMap for O(log n) price level lookups while maintaining
//! sorted order. This enables efficient spread calculation and market order
//! execution without full book traversal.

use crate::level::Level;
use crate::order::{Fill, Order, OrderId, Price, Quantity, Side};
use std::collections::BTreeMap;

/// Complete order book for one market.
///
/// Maintains separate bid and ask sides. Bids are indexed in reverse
/// (highest price first) to enable O(1) best-bid access via BTreeMap.
/// This architecture supports fast market order execution and spread
/// calculation without iteration over all price levels.
#[derive(Debug)]
pub struct OrderBook {
    /// Bid orders: BTreeMap with Price as key, Level as value
    /// Uses reverse ordering (highest price first)
    bids: BTreeMap<std::cmp::Reverse<Price>, Level>,

    /// Ask orders: BTreeMap with Price as key, Level as value
    /// Natural ordering (lowest price first)
    asks: BTreeMap<Price, Level>,
}

impl OrderBook {
    /// Create a new empty order book
    pub fn new() -> Self {
        OrderBook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    /// Add a limit order to the book, returning fills if matched
    /// Returns: (remaining_order, fills)
    pub fn add_limit_order(&mut self, mut order: Order) -> (Option<Order>, Vec<Fill>) {
        let mut fills = Vec::new();

        match order.side {
            Side::Bid => {
                // Try to match against ask orders
                self.match_against_asks(&mut order, &mut fills);

                // If still has remaining quantity, add to book
                if order.is_live() {
                    let level = self
                        .bids
                        .entry(std::cmp::Reverse(order.price))
                        .or_insert_with(Level::new);
                    level.add_order(order.clone());
                    Ok(order)
                } else {
                    Ok(order)
                }
                .ok();
            }
            Side::Ask => {
                // Try to match against bid orders
                self.match_against_bids(&mut order, &mut fills);

                // If still has remaining quantity, add to book
                if order.is_live() {
                    let level = self
                        .asks
                        .entry(order.price)
                        .or_insert_with(Level::new);
                    level.add_order(order.clone());
                    Ok(order)
                } else {
                    Ok(order)
                }
                .ok();
            }
        }

        (if order.is_live() { Some(order) } else { None }, fills)
    }

    /// Cancel an order by ID, returning the cancelled order if found
    pub fn cancel_order(&mut self, order_id: OrderId) -> Option<Order> {
        // Search bid side
        for level in self.bids.values_mut() {
            if let Some(order) = level.remove_order(order_id) {
                return Some(order);
            }
        }

        // Search ask side
        for level in self.asks.values_mut() {
            if let Some(order) = level.remove_order(order_id) {
                return Some(order);
            }
        }

        None
    }

    /// Modify an order's price and/or quantity
    /// Returns: true if modification successful
    pub fn modify_order(
        &mut self,
        order_id: OrderId,
        new_price: Option<Price>,
        new_quantity: Option<Quantity>,
    ) -> bool {
        // Find and remove the order
        if let Some(mut order) = self.cancel_order(order_id) {
            if let Some(price) = new_price {
                order.price = price;
            }
            if let Some(quantity) = new_quantity {
                order.quantity = quantity;
                order.remaining = quantity;
            }

            // Re-add the modified order
            let (_remaining, _fills) = self.add_limit_order(order);
            true
        } else {
            false
        }
    }

    /// Get the best bid price
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next().map(|r| r.0)
    }

    /// Get the best ask price
    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    /// Get the current spread
    pub fn spread(&self) -> Option<i64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => {
                if bid.0 < ask.0 {
                    Some((ask.0 - bid.0) as i64)
                } else {
                    None // Crossed market
                }
            }
            _ => None,
        }
    }

    /// Get total quantity at the best bid
    pub fn bid_quantity_at_best(&self) -> Quantity {
        self.bids
            .values()
            .next()
            .map(|l| l.total_quantity())
            .unwrap_or(Quantity(0))
    }

    /// Get total quantity at the best ask
    pub fn ask_quantity_at_best(&self) -> Quantity {
        self.asks
            .values()
            .next()
            .map(|l| l.total_quantity())
            .unwrap_or(Quantity(0))
    }

    /// Get order by ID (searches both sides)
    pub fn get_order(&self, order_id: OrderId) -> Option<Order> {
        for level in self.bids.values() {
            if let Some(order) = level.get_order(order_id) {
                return Some(order.clone());
            }
        }

        for level in self.asks.values() {
            if let Some(order) = level.get_order(order_id) {
                return Some(order.clone());
            }
        }

        None
    }

    /// Get total quantity on bid side
    pub fn total_bid_quantity(&self) -> Quantity {
        Quantity(
            self.bids
                .values()
                .map(|l| l.total_quantity().0)
                .sum(),
        )
    }

    /// Get total quantity on ask side
    pub fn total_ask_quantity(&self) -> Quantity {
        Quantity(
            self.asks
                .values()
                .map(|l| l.total_quantity().0)
                .sum(),
        )
    }

    /// Get number of price levels on bid side
    pub fn bid_levels(&self) -> usize {
        self.bids.len()
    }

    /// Get number of price levels on ask side
    pub fn ask_levels(&self) -> usize {
        self.asks.len()
    }

    // Private matching logic

    /// Match a bid order against the ask side
    fn match_against_asks(&mut self, order: &mut Order, fills: &mut Vec<Fill>) {
        while order.is_live() && !self.asks.is_empty() {
            // Get the best ask price
            let best_ask = *self.asks.keys().next().unwrap();

            // Check if we can match
            if order.price.0 < best_ask.0 {
                break; // No match possible at better prices
            }

            // Match against this ask level
            let level = self.asks.get_mut(&best_ask).unwrap();

            while order.is_live() && !level.is_empty() {
                if let Some(maker) = level.front_mut() {
                    let fill_qty = Quantity(std::cmp::min(
                        order.remaining.0,
                        maker.remaining.0,
                    ));

                    fills.push(Fill {
                        maker_id: maker.id,
                        taker_id: order.id,
                        price: maker.price,
                        quantity: fill_qty,
                    });

                    maker.reduce_remaining(fill_qty);
                    order.reduce_remaining(fill_qty);

                    if maker.is_filled() {
                        level.pop_front();
                    }
                } else {
                    break;
                }
            }

            // Remove empty level
            if level.is_empty() {
                self.asks.remove(&best_ask);
            }
        }
    }

    /// Match an ask order against the bid side
    fn match_against_bids(&mut self, order: &mut Order, fills: &mut Vec<Fill>) {
        while order.is_live() && !self.bids.is_empty() {
            // Get the best bid price (highest price)
            let best_bid = self.bids.keys().next().map(|r| r.0).unwrap();

            // Check if we can match
            if order.price.0 > best_bid.0 {
                break; // No match possible at better prices
            }

            // Match against this bid level
            let best_bid_key = std::cmp::Reverse(best_bid);
            let level = self.bids.get_mut(&best_bid_key).unwrap();

            while order.is_live() && !level.is_empty() {
                if let Some(maker) = level.front_mut() {
                    let fill_qty = Quantity(std::cmp::min(
                        order.remaining.0,
                        maker.remaining.0,
                    ));

                    fills.push(Fill {
                        maker_id: maker.id,
                        taker_id: order.id,
                        price: maker.price,
                        quantity: fill_qty,
                    });

                    maker.reduce_remaining(fill_qty);
                    order.reduce_remaining(fill_qty);

                    if maker.is_filled() {
                        level.pop_front();
                    }
                } else {
                    break;
                }
            }

            // Remove empty level
            if level.is_empty() {
                self.bids.remove(&best_bid_key);
            }
        }
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_order(id: u64, side: Side, price: u64, qty: u64, seq: u64) -> Order {
        Order::new(OrderId(id), side, Price(price), Quantity(qty), seq, 0)
    }

    #[test]
    fn test_simple_limit_match() {
        let mut book = OrderBook::new();

        // Add a bid
        let bid = create_order(1, Side::Bid, 10000000000, 100, 1);
        let (remaining, fills) = book.add_limit_order(bid);
        assert!(remaining.is_some());
        assert_eq!(fills.len(), 0);

        // Add matching ask
        let ask = create_order(2, Side::Ask, 10000000000, 100, 2);
        let (remaining, fills) = book.add_limit_order(ask);
        assert!(remaining.is_none()); // Fully filled
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].quantity.0, 100);
    }

    #[test]
    fn test_partial_fill() {
        let mut book = OrderBook::new();

        // Add a large bid
        let bid = create_order(1, Side::Bid, 10000000000, 100, 1);
        book.add_limit_order(bid);

        // Add smaller ask
        let ask = create_order(2, Side::Ask, 10000000000, 50, 2);
        let (remaining, fills) = book.add_limit_order(ask);
        assert!(remaining.is_none());
        assert_eq!(fills.len(), 1);

        // Check bid was partially filled
        let remaining_bid = book.get_order(OrderId(1)).unwrap();
        assert_eq!(remaining_bid.remaining.0, 50);
    }

    #[test]
    fn test_price_time_priority() {
        let mut book = OrderBook::new();

        // Add two bids at same price
        let bid1 = create_order(1, Side::Bid, 10000000000, 100, 1);
        let bid2 = create_order(2, Side::Bid, 10000000000, 100, 2);
        book.add_limit_order(bid1);
        book.add_limit_order(bid2);

        // Add small ask that matches both
        let ask = create_order(3, Side::Ask, 10000000000, 150, 3);
        let (_remaining, fills) = book.add_limit_order(ask);

        // First bid should fill first (FIFO)
        assert_eq!(fills[0].maker_id, OrderId(1));
        assert_eq!(fills[0].quantity.0, 100);
        assert_eq!(fills[1].maker_id, OrderId(2));
        assert_eq!(fills[1].quantity.0, 50);
    }

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new();

        let bid = create_order(1, Side::Bid, 10000000000, 100, 1);
        book.add_limit_order(bid);

        let cancelled = book.cancel_order(OrderId(1));
        assert!(cancelled.is_some());

        let cancelled_again = book.cancel_order(OrderId(1));
        assert!(cancelled_again.is_none());
    }

    #[test]
    fn test_modify_order() {
        let mut book = OrderBook::new();

        let bid = create_order(1, Side::Bid, 10000000000, 100, 1);
        book.add_limit_order(bid);

        assert!(book.modify_order(
            OrderId(1),
            Some(Price(11000000000)),
            Some(Quantity(150))
        ));

        let modified = book.get_order(OrderId(1)).unwrap();
        assert_eq!(modified.price.0, 11000000000);
        assert_eq!(modified.quantity.0, 150);
    }

    #[test]
    fn test_empty_book() {
        let book = OrderBook::new();
        assert_eq!(book.best_bid(), None);
        assert_eq!(book.best_ask(), None);
        assert_eq!(book.spread(), None);
    }
}
