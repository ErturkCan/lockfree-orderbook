use lockfree_orderbook::*;

#[test]
fn test_basic_limit_match() {
    let mut engine = MatchingEngine::new();

    // Add a bid
    let bid = engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    assert_eq!(bid.sequence, 1);
    assert_eq!(bid.fills.len(), 0);
    assert!(bid.remaining.is_some());

    // Add matching ask
    let ask = engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    assert_eq!(ask.sequence, 2);
    assert_eq!(ask.fills.len(), 1);
    assert!(ask.remaining.is_none());

    // Verify fill details
    let fill = &ask.fills[0];
    assert_eq!(fill.quantity.0, 100);
}

#[test]
fn test_partial_fill() {
    let mut engine = MatchingEngine::new();

    // Large bid
    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Smaller ask (partial fill)
    let ask = engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(100.0),
            quantity: Quantity(60),
        })
        .unwrap();

    assert_eq!(ask.fills.len(), 1);
    assert_eq!(ask.fills[0].quantity.0, 60);
    assert!(ask.remaining.is_none());

    // Check remaining bid
    let remaining_bid = engine.get_order(OrderId(1)).unwrap();
    assert_eq!(remaining_bid.remaining.0, 40);
}

#[test]
fn test_price_time_priority() {
    let mut engine = MatchingEngine::new();

    // Two bids at same price
    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Ask that matches both
    let ask = engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(100.0),
            quantity: Quantity(150),
        })
        .unwrap();

    assert_eq!(ask.fills.len(), 2);
    // First bid (order 1) should fill first due to FIFO
    assert_eq!(ask.fills[0].maker_id, OrderId(1));
    assert_eq!(ask.fills[0].quantity.0, 100);
    // Second bid (order 2) fills partially
    assert_eq!(ask.fills[1].maker_id, OrderId(2));
    assert_eq!(ask.fills[1].quantity.0, 50);
}

#[test]
fn test_cancel_order() {
    let mut engine = MatchingEngine::new();

    let bid = engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    let order_id = OrderId(bid.sequence);

    // Cancel should succeed
    let cancel = engine.process_event(OrderEvent::Cancel { order_id }).unwrap();
    assert_eq!(cancel.fills.len(), 0);

    // Order should no longer exist
    assert!(engine.get_order(order_id).is_none());

    // Trying to cancel again should fail
    let cancel_again = engine.process_event(OrderEvent::Cancel { order_id });
    assert!(cancel_again.is_err());
}

#[test]
fn test_modify_order() {
    let mut engine = MatchingEngine::new();

    let bid = engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    let order_id = OrderId(bid.sequence);

    // Modify price and quantity
    let modify = engine
        .process_event(OrderEvent::Modify {
            order_id,
            new_price: Some(Price::from_decimal(105.0)),
            new_quantity: Some(Quantity(150)),
        })
        .unwrap();

    assert!(modify.fills.is_empty());

    // Check modified order
    let modified = engine.get_order(order_id).unwrap();
    assert_eq!(modified.price.to_decimal(), 105.0);
    assert_eq!(modified.quantity.0, 150);
}

#[test]
fn test_empty_book() {
    let engine = MatchingEngine::new();

    assert_eq!(engine.best_bid(), None);
    assert_eq!(engine.best_ask(), None);
    assert_eq!(engine.spread(), None);
}

#[test]
fn test_market_order() {
    let mut engine = MatchingEngine::new();

    // Place multiple bids at different prices
    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(99.0),
            quantity: Quantity(100),
        })
        .unwrap();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Market sell should match against best bid
    let market = engine
        .process_event(OrderEvent::Market {
            side: Side::Ask,
            quantity: Quantity(150),
        })
        .unwrap();

    assert_eq!(market.fills.len(), 2);
    // Should match against best bid (100.0) first
    assert_eq!(market.fills[0].price.to_decimal(), 100.0);
}

#[test]
fn test_spread_calculation() {
    let mut engine = MatchingEngine::new();

    // Add bid
    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(99.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Add ask at different price
    engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(101.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Spread should be 2.0 (in fixed-point: 2 * 10^8)
    let spread = engine.spread().unwrap();
    assert_eq!(spread, 200000000);
}

#[test]
fn test_multiple_price_levels() {
    let mut engine = MatchingEngine::new();

    // Add multiple bid levels
    for i in 1..=5 {
        engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price::from_decimal(100.0 - i as f64),
                quantity: Quantity(100),
            })
            .unwrap();
    }

    // Add multiple ask levels
    for i in 1..=5 {
        engine
            .process_event(OrderEvent::Limit {
                side: Side::Ask,
                price: Price::from_decimal(100.0 + i as f64),
                quantity: Quantity(100),
            })
            .unwrap();
    }

    // Verify spread
    assert_eq!(engine.best_bid().unwrap().to_decimal(), 99.0);
    assert_eq!(engine.best_ask().unwrap().to_decimal(), 101.0);
}

#[test]
fn test_sequential_ordering() {
    let mut engine = MatchingEngine::new();

    // Process multiple events
    for i in 0..10 {
        let response = engine
            .process_event(OrderEvent::Limit {
                side: if i % 2 == 0 {
                    Side::Bid
                } else {
                    Side::Ask
                },
                price: Price::from_decimal(100.0),
                quantity: Quantity(100),
            })
            .unwrap();

        assert_eq!(response.sequence, (i + 1) as u64);
    }

    assert_eq!(engine.sequencer().total_sequenced(), 10);
}

#[test]
fn test_journal_recording() {
    let mut engine = MatchingEngine::new();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    let journal = engine.journal();
    assert_eq!(journal.order_event_count(), 2);
    assert_eq!(journal.fill_count(), 1);
}

#[test]
fn test_no_match_different_prices() {
    let mut engine = MatchingEngine::new();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(99.0),
            quantity: Quantity(100),
        })
        .unwrap();

    let ask = engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(101.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Should not match
    assert_eq!(ask.fills.len(), 0);
    assert!(ask.remaining.is_some());
}

#[test]
fn test_best_ask_logic() {
    let mut engine = MatchingEngine::new();

    // Add asks at different prices
    engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(102.0),
            quantity: Quantity(100),
        })
        .unwrap();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(101.0),
            quantity: Quantity(100),
        })
        .unwrap();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Ask,
            price: Price::from_decimal(103.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Best ask should be lowest price (101.0)
    assert_eq!(engine.best_ask().unwrap().to_decimal(), 101.0);
}

#[test]
fn test_best_bid_logic() {
    let mut engine = MatchingEngine::new();

    // Add bids at different prices
    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(99.0),
            quantity: Quantity(100),
        })
        .unwrap();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(100.0),
            quantity: Quantity(100),
        })
        .unwrap();

    engine
        .process_event(OrderEvent::Limit {
            side: Side::Bid,
            price: Price::from_decimal(98.0),
            quantity: Quantity(100),
        })
        .unwrap();

    // Best bid should be highest price (100.0)
    assert_eq!(engine.best_bid().unwrap().to_decimal(), 100.0);
}

#[test]
fn test_metrics_tracking() {
    let mut engine = MatchingEngine::new();

    for _ in 0..10 {
        engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price::from_decimal(100.0),
                quantity: Quantity(100),
            })
            .unwrap();
    }

    let metrics = engine.metrics();
    assert_eq!(metrics.total_orders(), 10);
}
