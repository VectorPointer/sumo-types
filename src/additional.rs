//! `additional` format: reads `.add.xml` (SUMO "additional" files), of
//! which only the E1/E2/E3 detector definitions are modelled, and (under
//! `write`) writes it back out. Own namespace at `sumo_types::additional` —
//! see the crate docs for why, like `routes` and unlike `net`, it isn't
//! flattened to the crate root.

pub mod domain;
pub mod reader;
pub mod schema_mapper;

pub use domain::Additional;
pub use reader::{read_additional, read_additional_from};
