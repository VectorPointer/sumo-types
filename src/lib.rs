//! Reads [SUMO](https://sumo.dlr.de/) road networks (`.net.xml`, as produced
//! by `netconvert`) into well-typed Rust structs.
//!
//! The crate follows a 3-layer model:
//!
//! 1. [`schema`] (generated at build time from the XSDs in `xsd/`) — an
//!    almost literal mirror of the SUMO schema, with no domain semantics.
//!    Not meant to be used directly.
//! 2. [`domain`] — this crate's own types ([`Network`], [`domain::Edge`],
//!    [`domain::Lane`], [`domain::Junction`], [`domain::Connection`], ...),
//!    independent of SUMO/XSD. This is what consumers build on.
//! 3. [`schema_mapper`] — converts layer 1 into layer 2, interpreting SUMO's
//!    text-encoded positions, shapes, boundaries, and enumerations along the
//!    way.
//!
//! The usual entry point is [`read_network`]:
//!
//! ```no_run
//! let network = sumo_net::read_network(std::path::Path::new("city.net.xml"))?;
//! println!("{} edges", network.edges.len());
//! # Ok::<(), anyhow::Error>(())
//! ```

/// Types generated from the SUMO XSDs by `build.rs`.
/// Layer 1: an (almost) literal mirror of the schema, with no domain semantics.
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
#[allow(non_camel_case_types, unused_variables, unused_mut, clippy::all)]
pub mod schema {
    include!(concat!(env!("OUT_DIR"), "/generated_schema.rs"));
}

/// Layer 2: this crate's own types, decoupled from SUMO/XSD.
pub mod domain;
/// Reading a `.net.xml` file into a [`Network`].
pub mod reader;
/// Conversion from layer 1 (schema) to layer 2 (domain).
pub mod schema_mapper;

pub use domain::Network;
pub use reader::{read_network, read_network_from};

/// Re-exported because it appears in this crate's public API
/// ([`domain::Lane::length`], [`domain::Lane::speed`], ...): consumers must
/// build their `Length`/`Velocity` values with the same `uom` version this
/// crate was compiled against, and going through `sumo_net::uom` guarantees
/// that without them having to track the version themselves.
pub use uom;
