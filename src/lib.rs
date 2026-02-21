//! Lock-free order matching engine for high-performance trading systems.
//!
//! This library provides a single-threaded, zero-allocation sequencer-based
//! order matching engine optimized for ultra-low latency. The architecture
//! separates ordering (sequencer) from matching (engine) to enable lock-free
//! concurrent order placement in the future while maintaining FIFO semantics.

pub mod book;
pub mod engine;
pub mod journal;
pub mod level;
pub mod metrics;
pub mod order;
pub mod sequencer;

pub use book::OrderBook;
pub use engine::MatchingEngine;
pub use journal::EventJournal;
pub use metrics::Metrics;
pub use order::{Order, OrderEvent, OrderId, Price, Quantity, Side};
pub use sequencer::Sequencer;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
