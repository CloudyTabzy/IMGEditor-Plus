//! Project-wide hash collections.
//!
//! `hashbrown`'s SwissTable with the crate-default foldhash. Preferred over
//! `std::collections` because the application's keys are never
//! attacker-controlled (entry names, indices, pixels), and foldhash drops
//! SipHash-1-3's cost — measured ~1.8–2.5× on the app's key shapes in
//! `examples/benchmark_hashmaps.rs` (local-only harness). `hashbrown` is
//! already in the dependency tree through `quick_cache`/`indexmap`, so this
//! adds no new compile units. Every user-visible ordering is produced by
//! explicit `Vec` sorts or `BTreeMap` output, so iteration order never leaks.
//! Decision record: `docs/hashmap-crate.md`.

pub use hashbrown::hash_map::Entry;
pub use hashbrown::{HashMap, HashSet};
