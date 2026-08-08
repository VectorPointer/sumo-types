//! Reads [SUMO](https://sumo.dlr.de/) files into well-typed Rust structs.
//!
//! Each SUMO file format is its own Cargo feature, generated from that
//! format's XSD; see `FEATURE_SCHEMAS` in `build.rs` for the current list.
//! Two are implemented:
//!
//! | Feature | Format | Public API |
//! |---|---|---|
//! | `net` (default) | `.net.xml` (`netconvert` output) | [`domain`], [`read_network`] |
//! | `routes` | `.rou.xml` (traffic demand) | [`routes`] |
//!
//! `net`'s API sits at the crate root (`sumo_types::domain::Network`,
//! `sumo_types::read_network`) because it was this crate's first, only
//! format; that path is kept stable even though `net`'s own files live
//! under `src/net/` internally — a module's file location and its public
//! path are independent in Rust, and `lib.rs` re-exports `net`'s public
//! items at the root on purpose. `routes`, added later, didn't inherit that
//! spot: it's namespaced at [`routes`] instead, both to avoid colliding
//! with `net`'s type names (both formats have their own `Route`-shaped
//! concepts: `net::domain` doesn't, but a third format might) and so the
//! module a type is imported from tells you which SUMO format it belongs
//! to.
//!
//! Every format has 2 layers of types and a conversion step between them.
//! For `net`:
//!
//! ```text
//! .net.xml --(xsd-parser, at build time)--> schema --(schema_mapper)--> domain
//! ```
//!
//! - [`schema`] (layer 1, generated at build time from the XSDs in `xsd/`)
//!   — an almost literal mirror of the SUMO schema, with no domain
//!   semantics. Not meant to be used directly. Shared by every enabled
//!   format (see `FEATURE_SCHEMAS` in `build.rs`), so `schema::NetType` and
//!   `schema::RoutesType` both live here when both features are on.
//! - `schema_mapper` converts layer 1 into layer 2, interpreting SUMO's
//!   text-encoded positions, shapes, boundaries, and enumerations along the
//!   way. For example, `schema::LaneType::shape` is a raw `String`
//!   (`"0.00,0.00 100.00,0.00"`); `net`'s `schema_mapper` turns it into a
//!   [`domain::Shape`] (a `Vec<`[`domain::Point`]`>`).
//! - `domain` (layer 2) — hand-written types independent of SUMO/XSD. This
//!   is what consumers build on: [`domain`] for `net`, [`routes::domain`]
//!   for `routes`. The two are unrelated Rust types even where SUMO's
//!   concepts overlap (a route's edges and a network's edges are both just
//!   ids, `domain::EdgeId` vs. `routes::domain::EdgeRef`) — `routes` doesn't
//!   require `net` to be enabled, so it can't borrow its types.
//!
//! `read_network` runs the whole pipeline for `net`, so consumers only ever
//! have to deal with layer 2 (see its own doc comment in `net/reader.rs`
//! for a runnable example) — marked `ignore` here rather than `no_run`
//! because this doc comment has no `#[cfg(feature = "net")]` to gate it
//! (crate-level `//!` docs can't be attached to a single item), so it has
//! to compile under every feature combination, including `routes` alone:
//!
//! ```ignore
//! let network = sumo_types::read_network(std::path::Path::new("city.net.xml"))?;
//! println!("{} edges", network.edges.len());
//! # Ok::<(), anyhow::Error>(())
//! ```

/// Types generated from the active features' XSDs by `build.rs` (see
/// `FEATURE_SCHEMAS` there). Layer 1: an (almost) literal mirror of the
/// schema, with no domain semantics.
///
/// Enum variants preserve the exact case of the XSD value (see the custom
/// `Naming` in `build.rs`) so information like `state="M"` vs. `state="m"`
/// isn't lost; that's why `non_camel_case_types` is disabled for the whole
/// module.
///
/// `clippy::all` is disabled here for the same reason: xsd-parser's output
/// isn't hand-written, so its style lints (redundant field names, collapsible
/// `if`s, ...) are noise nobody can act on — and drowned out the handful of
/// real ones in the rest of the crate.
///
/// `dead_code` is disabled too: every enabled format's *entire* schema is
/// generated here (see `FEATURE_SCHEMAS` in `build.rs`), but each format's
/// `domain`/`schema_mapper` only maps the subset it actually models — e.g.
/// `routes` doesn't map `vTypeType`'s car-following-model choice, so
/// xsd-parser's generated deserializer state for that choice is never
/// constructed. That's expected, not a bug to silence one warning at a time.
#[allow(non_camel_case_types, unused_variables, unused_mut, dead_code, clippy::all)]
pub mod schema {
    include!(concat!(env!("OUT_DIR"), "/generated_schema.rs"));
}

/// `net` format's own files (`domain`, `reader`, `schema_mapper`); kept
/// private and re-exported below so its public path (`sumo_types::domain`,
/// `sumo_types::read_network`) doesn't have to match where the files live.
/// See the crate docs for why `routes` isn't re-exported the same way.
///
/// `#[doc(hidden)]`: without it, rustdoc still generates full doc pages at
/// `net`'s own (unreachable — `net` isn't `pub`) path, e.g.
/// `sumo_types::net::domain::Network`, duplicating `sumo_types::domain::Network`
/// with a URL that doesn't actually compile if used. Hiding the module
/// stops that; the `pub use` below still inlines everything at the real
/// public path.
#[cfg(feature = "net")]
#[doc(hidden)]
mod net;
#[cfg(feature = "net")]
pub use net::domain;
#[cfg(feature = "net")]
pub use net::domain::Network;
#[cfg(feature = "net")]
pub use net::reader;
#[cfg(feature = "net")]
pub use net::reader::{read_network, read_network_from};
#[cfg(feature = "net")]
pub use net::schema_mapper;

/// `routes` format: reads `.rou.xml`. Namespaced here (rather than
/// re-exported at the crate root like `net`) — see the crate docs.
#[cfg(feature = "routes")]
pub mod routes;

/// Re-exported because it appears in this crate's public API
/// ([`domain::Lane::length`], [`domain::Lane::speed`], ...): consumers must
/// build their `Length`/`Velocity` values with the same `uom` version this
/// crate was compiled against, and going through `sumo_types::uom` guarantees
/// that without them having to track the version themselves.
pub use uom;
