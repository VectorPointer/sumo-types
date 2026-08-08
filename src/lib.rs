//! Reads [SUMO](https://sumo.dlr.de/) files into well-typed Rust structs.
//!
//! Each SUMO file format is its own Cargo feature, generated from that
//! format's XSD; see `FEATURE_SCHEMAS` in `build.rs` for the current list.
//! Two are implemented:
//!
//! | Feature | Format | Public API |
//! |---|---|---|
//! | `net` (default) | `.net.xml` (`netconvert` output) | `domain`, `read_network` |
//! | `routes` | `.rou.xml` (traffic demand) | `routes` |
//!
//! Those are written as plain code spans, not intra-doc links, throughout
//! this crate-level comment: it can't be `#[cfg]`-gated (a `//!` comment
//! isn't attached to a single item), so it is rendered under every feature
//! combination, and a link to a feature-gated item is a broken link in every
//! build that doesn't enable it. Everything named here is reachable from the
//! module list below under the features that provide it.
//!
//! `net`'s API sits at the crate root (`sumo_types::domain::Network`,
//! `sumo_types::read_network`) because it was this crate's first, only
//! format; that path is kept stable even though `net`'s own files live
//! under `src/net/` internally — a module's file location and its public
//! path are independent in Rust, and `lib.rs` re-exports `net`'s public
//! items at the root on purpose. `routes`, added later, didn't inherit that
//! spot: it's namespaced at `sumo_types::routes` instead, both to avoid colliding
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
//! - `schema` (layer 1, generated at build time from the XSDs in `xsd/`)
//!   — an almost literal mirror of the SUMO schema, with no domain
//!   semantics. **Private**: it is an implementation detail, and keeping
//!   it so is what keeps `xsd-parser-types` out of this crate's public
//!   API (see the module's own comment in `lib.rs`). Shared by every
//!   enabled format (see `FEATURE_SCHEMAS` in `build.rs`), so
//!   `schema::NetType` and `schema::RoutesType` both live there when both
//!   features are on.
//! - `schema_mapper` converts layer 1 into layer 2, interpreting SUMO's
//!   text-encoded positions, shapes, boundaries, and enumerations along the
//!   way. For example, `schema::LaneType::shape` is a raw `String`
//!   (`"0.00,0.00 100.00,0.00"`); `net`'s `schema_mapper` turns it into a
//!   `domain::Shape` (a `Vec<domain::Point>`).
//! - `domain` (layer 2) — hand-written types independent of SUMO/XSD. This
//!   is what consumers build on: `domain` for `net`, `routes::domain`
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

#[cfg(not(any(feature = "net", feature = "routes")))]
compile_error!(
    "sumo-types needs at least one format feature enabled: `net` (reads .net.xml) \
     or `routes` (reads .rou.xml). With none of them the crate has no schema to \
     generate and no API to offer."
);

/// Type-checks the examples in `README.md`, so they can't drift from the
/// real API. Only exists while running doctests, and only when every format
/// the README demonstrates is available — the file is one document, so it
/// can't be `#[cfg]`-gated feature by feature.
#[cfg(all(doctest, feature = "net", feature = "routes"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeExamples;

/// Types generated from the active features' XSDs by `build.rs` (see
/// `FEATURE_SCHEMAS` there). Layer 1: an (almost) literal mirror of the
/// schema, with no domain semantics.
///
/// **Private on purpose.** The generated types implement `xsd-parser-types`'
/// traits (`WithDeserializer`, `DeserializeBytes`, ...) and name its `Error`
/// in 173 impls, so making this module `pub` would put `xsd-parser-types`
/// into this crate's public API. Traits are identified by their defining
/// crate *version*: `WithDeserializer` from 0.2 and from 0.3 are different
/// traits, so bumping that dependency would break every consumer who built
/// on layer 1 — a breaking release forced by a dependency bump that has
/// nothing to do with this crate's own API. Layer 2 (`domain`) doesn't
/// mention `xsd-parser-types` anywhere, so keeping this module private is
/// what lets the dependency stay an implementation detail.
///
/// Contrast [`uom`], which *is* re-exported: `domain::Lane::length` is a
/// `Length`, so that one is public API by design rather than by accident.
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
#[allow(
    non_camel_case_types,
    unused_variables,
    unused_mut,
    dead_code,
    clippy::all
)]
mod schema {
    include!(concat!(env!("OUT_DIR"), "/generated_schema.rs"));
}

/// XML plumbing shared by every format's reader: checking that a document's
/// root element is the one that format expects, which xsd-parser's generated
/// deserializers don't do. Only needed when there is a reader to plumb.
#[cfg(any(feature = "net", feature = "routes"))]
mod xml;

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
/// (`domain::Lane::length`, `domain::Lane::speed`, ... — code spans rather
/// than links, because `domain` only exists under the `net` feature while
/// this re-export is unconditional): consumers must
/// build their `Length`/`Velocity` values with the same `uom` version this
/// crate was compiled against, and going through `sumo_types::uom` guarantees
/// that without them having to track the version themselves.
pub use uom;

// `xsd-parser-types` is deliberately *not* re-exported, unlike `uom`. It
// appears nowhere in this crate's public API — only inside the private
// `schema` module and the two readers' bodies — so re-exporting it would
// hand back exactly the semver lock that keeping `schema` private buys:
// a `pub use` of a dependency makes bumping that dependency a breaking
// change for anyone who reached it through this crate.
