use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lockfree_orderbook::*;

fn bench_single_order(c: &mut Criterion) {
    c.bench_function("single_limit_order", |b| {
        b.iter(|| {
            let mut engine = MatchingEngine::new();
            engine.process_event(OrderEvent::Limit {
                side: black_box(Side::Bid),
                price: black_box(Price(10000000000)),
                quantity: black_box(Quantity(100)),
            })
        })
    });
}

fn bench_order_matching(c: &mut Criterion) {
    c.bench_function("order_matching", |b| {
        b.iter(|| {
            let mut engine = MatchingEngine::new();

            engine.process_event(OrderEvent::Limit {
                side: black_box(Side::Bid),
                price: black_box(Price(10000000000)),
                quantity: black_box(Quantity(100)),
            })
            .unwrap();

            engine.process_event(OrderEvent::Limit {
                side: black_box(Side::Ask),
                price: black_box(Price(10000000000)),
                quantity: black_box(Quantity(100)),
            })
        })
    });
}

fn bench_order_insertion_with_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_insertion_depth");

    for depth in [10, 100, 1000].iter() {
        group.bench_with_input(
            format!("depth_{}", depth),
            depth,
            |b, &depth| {
                b.iter(|| {
                    let mut engine = MatchingEngine::new();

                    // Build order book with depth
                    for i in 0..depth {
                        engine
                            .process_event(OrderEvent::Limit {
                                side: Side::Bid,
                                price: Price(10000000000 - (i as u64 * 100000)),
                                quantity: Quantity(100),
                            })
                            .unwrap();
                    }

                    // Insert new order
                    engine.process_event(OrderEvent::Limit {
                        side: black_box(Side::Bid),
                        price: black_box(Price(9000000000)),
                        quantity: black_box(Quantity(100)),
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_cancellation(c: &mut Criterion) {
    c.bench_function("cancel_order", |b| {
        b.iter_with_setup(
            || {
                let mut engine = MatchingEngine::new();
                engine
                    .process_event(OrderEvent::Limit {
                        side: Side::Bid,
                        price: Price(10000000000),
                        quantity: Quantity(100),
                    })
                    .unwrap();
                engine
            },
            |mut engine| {
                engine.process_event(OrderEvent::Cancel {
                    order_id: black_box(OrderId(1)),
                })
            },
        )
    });
}

fn bench_market_order(c: &mut Criterion) {
    c.bench_function("market_order", |b| {
        b.iter_with_setup(
            || {
                let mut engine = MatchingEngine::new();
                engine
                    .process_event(OrderEvent::Limit {
                        side: Side::Bid,
                        price: Price(10000000000),
                        quantity: Quantity(1000),
                    })
                    .unwrap();
                engine
            },
            |mut engine| {
                engine.process_event(OrderEvent::Market {
                    side: black_box(Side::Ask),
                    quantity: black_box(Quantity(100)),
                })
            },
        )
    });
}

criterion_group!(
    benches,
    bench_single_order,
    bench_order_matching,
    bench_order_insertion_with_depth,
    bench_cancellation,
    bench_market_order
);
criterion_main!(benches);
