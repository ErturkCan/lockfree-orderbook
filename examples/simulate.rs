use lockfree_orderbook::*;
use rand::Rng;
use std::time::Instant;

/// Realistic order flow simulation
fn simulate_trading(order_count: usize) {
    println!(
        "=== Order Matching Engine Simulation ===\n\
         Generating {} orders...\n",
        order_count
    );

    let mut engine = MatchingEngine::new();
    let mut rng = rand::thread_rng();

    // Base price around 100
    let base_price = Price::from_decimal(100.0);
    let base_price_fixed = 10000000000u64;

    // Pre-populate with some liquidity
    println!("Pre-populating order book with initial liquidity...");
    for i in 1..=10 {
        engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(base_price_fixed - (i as u64 * 100000000)), // Spread by 0.1
                quantity: Quantity(100 * i),
            })
            .unwrap();

        engine
            .process_event(OrderEvent::Limit {
                side: Side::Ask,
                price: Price(base_price_fixed + (i as u64 * 100000000)), // Spread by 0.1
                quantity: Quantity(100 * i),
            })
            .unwrap();
    }

    // Process random orders
    let start = Instant::now();

    for i in 0..order_count {
        let event_type = rng.gen_range(0..100);

        let event = if event_type < 60 {
            // 60% limit orders
            let side = if rng.gen_bool(0.5) {
                Side::Bid
            } else {
                Side::Ask
            };

            // Random price near mid
            let offset = (rng.gen_range(-10..=10) as i64 as u64).wrapping_mul(10000000);
            let price = base_price_fixed.wrapping_add(offset);

            OrderEvent::Limit {
                side,
                price: Price(price),
                quantity: Quantity(rng.gen_range(10..500)),
            }
        } else if event_type < 80 {
            // 20% market orders
            let side = if rng.gen_bool(0.5) {
                Side::Bid
            } else {
                Side::Ask
            };

            OrderEvent::Market {
                side,
                quantity: Quantity(rng.gen_range(5..100)),
            }
        } else {
            // 20% cancellations
            if i > 20 {
                let cancel_id = rng.gen_range(1..(i as u64));
                OrderEvent::Cancel {
                    order_id: OrderId(cancel_id),
                }
            } else {
                continue;
            }
        };

        match engine.process_event(event) {
            Ok(response) => {
                if !response.fills.is_empty() && i % 100 == 0 {
                    println!(
                        "[Order {}] Generated {} fills",
                        response.sequence,
                        response.fills.len()
                    );
                }
            }
            Err(e) => {
                println!("Error processing order: {}", e);
            }
        }
    }

    let elapsed = start.elapsed();

    // Print results
    println!("\n=== Simulation Complete ===\n");
    println!("Time elapsed: {:.3}s", elapsed.as_secs_f64());
    println!("Orders processed: {}", order_count);
    println!(
        "Throughput: {:.0} orders/sec",
        order_count as f64 / elapsed.as_secs_f64()
    );

    // Print final market state
    println!("\n=== Final Market State ===");
    if let Some(bid) = engine.best_bid() {
        println!("Best Bid: {}", bid);
    }

    if let Some(ask) = engine.best_ask() {
        println!("Best Ask: {}", ask);
    }

    if let Some(spread) = engine.spread() {
        println!("Spread: {} (fixed-point)", spread);
    }

    println!("Bid Quantity: {}", engine.bid_quantity());
    println!("Ask Quantity: {}", engine.ask_quantity());
    println!("Total Orders in Book: {}", engine.total_orders_in_book());

    // Print metrics
    println!("\n{}", engine.metrics().report());

    // Print journal stats
    let journal = engine.journal();
    println!(
        "Journal Events: {} (orders: {}, fills: {})",
        journal.len(),
        journal.order_event_count(),
        journal.fill_count()
    );
}

/// Analyze latency distribution
fn analyze_latency(order_count: usize) {
    println!("=== Latency Analysis ===\n");

    let mut engine = MatchingEngine::new();
    let mut rng = rand::thread_rng();
    let base_price = 10000000000u64;

    // Build book
    for i in 0..50 {
        engine
            .process_event(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(base_price - (i as u64 * 100000)),
                quantity: Quantity(100),
            })
            .unwrap();
    }

    // Process orders and track latencies
    for _ in 0..order_count {
        let side = if rng.gen_bool(0.5) {
            Side::Bid
        } else {
            Side::Ask
        };

        let _ = engine.process_event(OrderEvent::Limit {
            side,
            price: Price(base_price),
            quantity: Quantity(rng.gen_range(10..100)),
        });
    }

    let metrics = engine.metrics();
    println!("Order Processing Latencies:");
    println!("  Min: {:.3} µs", metrics.order_latency().min_ns as f64 / 1000.0);
    println!(
        "  Max: {:.3} µs",
        metrics.order_latency().max_ns as f64 / 1000.0
    );
    println!("  Avg: {:.3} µs", metrics.order_latency().avg_ns() / 1000.0);
}

fn main() {
    println!("Lock-Free Order Matching Engine - Simulation Examples\n");

    // Run main simulation
    simulate_trading(10000);

    println!("\n---\n");

    // Run latency analysis
    analyze_latency(1000);
}
