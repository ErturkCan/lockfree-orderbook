//! Price level implementation using FIFO matching.
//!
//! Each price level maintains a queue of orders at that price level.
//! When matching occurs, orders are filled in FIFO order (price-time priority).
//! This module uses VecDeque to maintain insertion order while enabling
//! efficient removal from the front.

use crate::order::{Order, OrderId, Quantity};
use std::collections::VecDeque;

/// A price level containing orders at a specific price point.
///
/// Maintains FIFO ordering via VecDeque. When a market order arrives,
/// orders at this level are filled in FIFO order.
#[derive(Debug)]
pub struct Level {
    /// Queue of orders at this price level, in FIFO order
    orders: VecDeque<Order>,
}

impl Level {
    /// Create a new empty price level
    pub fn new() -> Self {
        Level {
            orders: VecDeque::new(),
        }
    }

    /// Add an order to this level
    pub fn add_order(&mut self, order: Order) {
        self.orders.push_back(order);
    }

    /// Remove an order by its ID, returning the order if found
    pub fn remove_order(&mut self, order_id: OrderId) -> Option<Order> {
        if let Some(pos) = self.orders.iter().position(|o| o.id == order_id) {
            self.orders.remove(pos)
        } else {
            None
        }
    }

    /// Get a mutable reference to the first (front) order
    pub fn front_mut(&mut self) -> Option<&mut Order> {
        self.orders.front_mut()
    }

    /// Get an immutable reference to the first order
    pub fn front(&self) -> Option<&Order> {
        self.orders.front()
    }

    /// Remove and return the first (front) order
    pub fn pop_front(&mut self) -> Option<Order> {
        self.orders.pop_front()
    }

    /// Get an order by ID
    pub fn get_order(&self, order_id: OrderId) -> Option<&Order> {
        self.orders.iter().find(|o| o.id == order_id)
    }

    /// Get a mutable order by ID
    pub fn get_order_mut(&mut self, order_id: OrderId) -> Option<&mut Order> {
        self.orders.iter_mut().find(|o| o.id == order_id)
    }

    /// Get total quantity available at this level
    pub fn total_quantity(&self) -> Quantity {
        Quantity(self.orders.iter().map(|o| o.remaining.0).sum())
    }

    /// Check if this level is empty
    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    /// Get number of orders at this level
    pub fn len(&self) -> usize {
        self.orders.len()
    }

    /// Iterate over orders at this level
    pub fn iter(&self) -> impl Iterator<Item = &Order> {
        self.orders.iter()
    }

    /// Get mutable iterator over orders
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Order> {
        self.orders.iter_mut()
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::order::{OrderId, Price, Side};

    #[test]
    fn test_level_add_remove() {
        let mut level = Level::new();
        assert!(level.is_empty());

        let order = Order::new(
            OrderId(1),
            Side::Bid,
            Price(10000000000),
            Quantity(100),
            1,
            0,
        );
        level.add_order(order);
        assert_eq!(level.len(), 1);
        assert!(!level.is_empty());

        let removed = level.remove_order(OrderId(1));
        assert!(removed.is_some());
        assert!(level.is_empty());
    }

    #[test]
    fn test_level_fifo_order() {
        let mut level = Level::new();

        for i in 1..=3 {
            let order = Order::new(
                OrderId(i),
                Side::Bid,
                Price(10000000000),
                Quantity(100),
                i,
                0,
            );
            level.add_order(order);
        }

        assert_eq!(level.front().map(|o| o.id), Some(OrderId(1)));

        let popped = level.pop_front();
        assert_eq!(popped.map(|o| o.id), Some(OrderId(1)));
        assert_eq!(level.front().map(|o| o.id), Some(OrderId(2)));
    }

    #[test]
    fn test_level_total_quantity() {
        let mut level = Level::new();

        for i in 1..=3 {
            let order = Order::new(
                OrderId(i),
                Side::Bid,
                Price(10000000000),
                Quantity(100 * i),
                i,
                0,
            );
            level.add_order(order);
        }

        assert_eq!(level.total_quantity().0, 600);
    }
}
