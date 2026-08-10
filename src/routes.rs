//! `routes` format: reads `.rou.xml` (traffic demand: routes, vehicle
//! types, vehicles), and (under `write`) writes it back out. Own namespace
//! at `sumo_types::routes` — see the crate docs for why, unlike `net`, it
//! isn't flattened to the crate root.

pub mod domain;
pub mod reader;
pub mod schema_mapper;
#[cfg(feature = "write")]
pub mod schema_writer;
#[cfg(feature = "write")]
pub mod writer;

pub use domain::Routes;
pub use reader::{read_routes, read_routes_from};
#[cfg(feature = "write")]
pub use writer::{write_routes, write_routes_to};
