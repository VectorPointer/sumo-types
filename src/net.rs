//! `net` format: reads `.net.xml` (`netconvert` output).
//!
//! This module is private (see `lib.rs`) — its `domain`, `reader`, and
//! `schema_mapper` are re-exported at the crate root instead of under
//! `sumo_types::net::...`, so this file's own location doesn't leak into
//! the public API. `routes` (a later, independent format) is public at
//! `sumo_types::routes` instead: its own path, chosen when it was added, so
//! it doesn't collide with `net`'s already-established one.

// `#[doc(hidden)]` on each: without it, rustdoc still generates full doc
// pages at their own path (e.g. `sumo_types::net::domain::Network`), on top
// of the `pub use`-inlined ones at the real public path
// (`sumo_types::domain::Network`) — a duplicate, unreachable-from-code URL.
// `#[doc(hidden)]` on the outer `mod net;` in `lib.rs` alone isn't enough
// to stop that; it has to be on these too.
#[doc(hidden)]
pub mod domain;
#[doc(hidden)]
pub mod reader;
#[doc(hidden)]
pub mod schema_mapper;
