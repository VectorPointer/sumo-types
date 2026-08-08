//! `routes` format: reads `.rou.xml` (traffic demand: routes, vehicle
//! types, vehicles). Own namespace at `sumo_types::routes` — see the crate
//! docs for why, unlike `net`, it isn't flattened to the crate root.

pub mod domain;
pub mod reader;
pub mod schema_mapper;

pub use domain::Routes;
pub use reader::{read_routes, read_routes_from};
