# Bolt's Journal - Critical Learnings Only

## 2025-05-18 - Full Cargo Test Suite Timeout & Search Key Optimization
**Learning:** Running `cargo test` without filters or compiling full release benchmarks takes a very long time and can hit timeout limits because of the massive workspace dependency graph (e.g. `waproto`, `whatsapp-rust`, audio libraries). In `src/util.rs`, `search_key()` performs accent-stripping search key normalization using `icu_properties::CodePointMapData::<GeneralCategory>::new()`. Calling `new()` repeatedly inside the iterator filter closure creates the code point map lookup data repeatedly per non-ASCII character, instead of instantiating it once outside the closure or iterator chain.
**Action:** Always filter tests specifically (e.g., `cargo test --lib -- util`), and hoist ICU lookup map instantiation outside per-character closures in character processing loops.
