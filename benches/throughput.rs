use criterion::{criterion_group, criterion_main, Criterion};
use lockfree_orderbook::*;
use rand::Rng;

/// Generate random order flow with realistic parameters
fn generate_order_flow(count: usize) -> Vec<OrderEvent> {
    let mut rng = rand::thread_rng();
    let mut orders = Vec::with_capacity(count);

    // Prices around 100 with some spread
    let base_price = 10000000000u64; // 100.0
    let spread = 50000000u64; // 0.5

    for i in 0..count {
        let event_type = rng.gen_range(0..100);

        if i < 10 {
            // Ensure we have some liquidity early
            orders.push(OrderEvent::Limit {
                side: Side::Bid,
                price: Price(base_price - spread),
                quantity: Quantity(rng.gen_range(10..1000)),
            });
        } else if event_type < 70 {
            // 70% limit orders
            let side = if rng.gen_bool(0.5) {
                Side::Bid
            } else {
                Side::Ask
            };

            let price = base_price
                + (rng.gen_range(-5..=5) as i64 as u64).wrapping_mul(10000000);

            orders.push(OrderEvent::Limit {
                side,
                price: Price(price),
                quantity: Quantity(rng.gen_range(10..500)),
            });
        } else if event_type < 85 {
            // 15% market orders
            let side = if rng.gen_bool(0.5) {
                Side::Bid
            } else {
                Side::Ask
            };

            orders.push(OrderEvent::Market {
                side,
                quantity: Quantity(rng.gen_range(5..100)),
            });
        } else {
            // 15% cancellations (of recent orders)
            if i > 10 {
                let cancel_id = rng.gen_range(1..(i as u64));
                orders.push(OrderEvent::Cancel {
                    order_id: OrderId(cancel_id),
                });
            }
        }
    }

    orders
}

fn bench_sustained_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("sustained_throughput");
    group.sample_size(10); // Reduce sample size for large benches

    for order_count in [100, 1000, 10000].iter() {
        group.bench_with_input(
            format!("orders_{}", order_count),
            order_count,
            |b, &order_count| {
                let orders = generate_order_flow(order_count);

                b.iter(|| {
                    let mut engine = MatchingEngine::new();

                    for order in &orders {
                        let _ = engine.process_event(order.clone());
                    }

                    // Return metrics for inspection
                    let metrics = engine.metrics();
                    (
                        metrics.total_orders(),
                        metrics.total_fills(),
                        metrics.order_latency().avg_ns(),
                    )
                })
            },
        );
    }

    group.finish();
}

fn bench_high_frequency_trading(c: &mut Criterion) {
    let mut group = c.benchmark_group("high_frequency");
    group.sample_size(10);

    group.bench_function("rapid_fire_orders", |b| {
        b.iter_with_setup(
            || {
                let mut engine = MatchingEngine::new();

                // Set up initial liquidity
                for i in 0..100 {
                    let _ = engine.process_event(OrderEvent::Limit {
                        side: Side::Bid,
                        price: Price(10000000000 - (i as u64 * 100000)),
                        quantity: Quantity(1000),
                    });
                }

                for i in 0..100 {
                    let _ = engine.process_event(OrderEvent::Limit {
                        side: Side::Ask,
                        price: Price(10000000000 + (i as u64 * 100000)),
                        quantity: Quantity(1000),
                    });
                }

                engine
            },
            |mut engine| {
                // Rapid market orders
                for _ in 0..1000 {
                    let _ = engine.process_event(OrderEvent::Market {
                        side: Side::Ask,
                        quantity: Quantity(10),
                    });
                }

                engine.metrics().total_fills()
            },
        )
    });

    group.finish();
}

fn bench_deep_book_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("deep_book");
    group.sample_size(10);

    group.bench_function("operations_with_10k_levels", |b| {
        b.iter_with_setup(
            || {
                let mut engine = MatchingEngine::new();

                // Build a deep book
                for i in 0..5000 {
                    let _ = engine.process_event(OrderEvent::Limit {
                        side: Side::Bid,
                        price: Price(10000000000 - (i as u64 * 1000)),
                        quantity: Quantity(100),
                    });
                }

                for i in 0..5000 {
                    let _ = engine.process_event(OrderEvent::Limit {
                        side: Side::Ask,
                        price: Price(10000000000 + (i as u64 * 1000)),
                        quantity: Quantity(100),
                    });
                }

                engine
            },
            |mut engine| {
                // Insert orders into the deep book
                for i in 0..100 {
                    let side = if i % 2 == 0 { Side::Bid } else { Side::Ask };
                    let _ = engine.process_event(OrderEvent::Limit {
                        side,
                        price: Price(10000000000),
                        quantity: Quantity(100),
                    });
                }

                engine.metrics().total_orders()
            },
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sustained_throughput,
    bench_high_frequency_trading,
    bench_deep_book_operations
);
criterion_main!(benches);
