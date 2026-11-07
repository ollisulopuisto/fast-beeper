# Bolt's Journal - Critical Learnings Only

## 2026-11-02 - ICU Bidi Property Lookups and Code Point Map Caching
**Learning:** In `src/bidi.rs`, `is_strong_rtl()` is called frequently across characters during galley text layout and rendering. Directly instantiating ICU's `CodePointMapData::<BidiClass>::new()` on each character call introduces high overhead. Furthermore, by Unicode specification, no code point below U+0590 possesses BidiClass `RightToLeft` or `ArabicLetter`. Filtering with `c < '\u{0590}'` allows all ASCII, Latin, Greek, and Cyrillic characters to exit immediately without any ICU lookup, and caching `CodePointMapDataBorrowed::<BidiClass>` via `OnceLock` eliminates repeated table instantiation for RTL code points.
**Action:** Use script/block boundaries (such as `c < '\u{0590}'` for RTL) to fast-path Unicode character classification, and cache ICU borrowed code point maps in a static `OnceLock` across the application lifecycle.

## 2025-05-18 - Full Cargo Test Suite Timeout & Search Key Optimization
**Learning:** Running `cargo test` without filters or compiling full release benchmarks takes a very long time and can hit timeout limits because of the massive workspace dependency graph (e.g. `waproto`, `whatsapp-rust`, audio libraries). In `src/util.rs`, `search_key()` performs accent-stripping search key normalization using `icu_properties::CodePointMapData::<GeneralCategory>::new()`. Calling `new()` repeatedly inside the iterator filter closure creates the code point map lookup data repeatedly per non-ASCII character, instead of instantiating it once outside the closure or iterator chain.
**Action:** Always filter tests specifically (e.g., `cargo test --lib -- util`), and hoist ICU lookup map instantiation outside per-character closures in character processing loops.

## 2026-11-07 - solmu-reader client registry cache
**Learning:** `serve_connection` in `src/read_http.rs` called `Registry::load` (disk read, JSON parse, and validation of every client entry) on every request, costing ~33-121 µs per request depending on registry size. Caching the parsed registry and reloading only when the file's mtime changes drops warm requests to ~3.8 µs (~15-30x, independent of registry size). `Registry::save` rewrites via temp file plus rename, so every add, revoke, or expiry bumps the mtime and forces a reload, keeping revocation immediate on the next request.
**Action:** Cache parsed configs keyed by file mtime only where the writer always renames into place (giving an invalidation signal); never cache without one.
