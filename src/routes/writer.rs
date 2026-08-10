//! Writing a [`Routes`] to a `.rou.xml` file: converts layer 2 into layer 1
//! (the private `schema` module) via [`super::schema_writer`] and
//! serializes it in one step, mirroring `routes`'s own `reader`.

use super::domain::Routes;
use crate::Result;
use crate::schema;
use crate::xml::{write_document, write_document_at};
use std::io::Write;
use std::path::Path;

/// Writes `routes` to the `.rou.xml` file at `path`, creating or truncating
/// it.
///
/// ```no_run
/// use sumo_types::routes::domain::{Depart, Route, RouteId, Routes, Vehicle, VehicleId};
/// use std::path::Path;
///
/// let routes = Routes {
///     routes: vec![Route { id: RouteId("r0".into()), edges: None, color: None }],
///     vehicles: vec![Vehicle {
///         id: VehicleId("v0".into()),
///         vehicle_type: None,
///         route: Some(RouteId("r0".into())),
///         depart: Depart::Begin,
///         color: None,
///     }],
///     ..Routes::default()
/// };
/// sumo_types::routes::write_routes(Path::new("city.rou.xml"), &routes)?;
/// # Ok::<(), sumo_types::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if `path` can't be created, or if writing to it fails.
pub fn write_routes(path: &Path, routes: &Routes) -> Result<()> {
    write_document_at::<schema::RoutesType, _>(routes, "routes", "SUMO route file", path)
}

/// Same as [`write_routes`], for callers that want the `.rou.xml` bytes
/// somewhere other than a file on disk (an in-memory buffer, a socket, ...).
///
/// # Errors
///
/// Same as [`write_routes`], minus the failure to create a file.
pub fn write_routes_to(routes: &Routes, sink: impl Write) -> Result<()> {
    write_document::<schema::RoutesType, _, _>(routes, "routes", "SUMO route file", sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::domain::{Color, Depart, NamedColor, Route, RouteId, Vehicle, VehicleId};
    use crate::routes::reader::read_routes_from;

    fn sample_routes() -> Routes {
        Routes {
            vehicle_types: vec![],
            routes: vec![Route {
                id: RouteId("r0".into()),
                edges: None,
                color: Some(Color::Named(NamedColor::Grey)),
            }],
            vehicles: vec![Vehicle {
                id: VehicleId("v0".into()),
                vehicle_type: None,
                route: Some(RouteId("r0".into())),
                depart: Depart::Begin,
                color: None,
            }],
        }
    }

    #[test]
    fn writes_a_document_a_correctly_rooted_reader_accepts() {
        let mut buf = Vec::new();
        write_routes_to(&sample_routes(), &mut buf).unwrap();

        let xml = String::from_utf8(buf).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<routes"));

        // Doesn't just parse — round-trips to the same value, which is the
        // property this feature actually exists for.
        let read_back = read_routes_from(xml.as_bytes()).unwrap();
        assert_eq!(read_back, sample_routes());
    }
}
