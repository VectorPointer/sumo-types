//! Reading a `.rou.xml` file into [`Routes`]: deserializes the input into
//! layer 1 (the private `schema` module) and converts it to layer 2
//! ([`crate::routes::domain`]) in one step, mirroring
//! `net`'s own `reader`.

use super::domain::Routes;
use crate::Result;
use crate::schema;
use crate::xml::{read_document, read_document_at};
use std::io::BufRead;
use std::path::Path;

/// Reads and deserializes the `.rou.xml` file at `path` into [`Routes`].
///
/// # Errors
///
/// Returns an error if `path` can't be opened, if the document isn't
/// well-formed XML rooted at `<routes>`, or if any of its attributes can't be
/// interpreted.
pub fn read_routes(path: &Path) -> Result<Routes> {
    read_document_at::<schema::RoutesType, _>(path, "routes", "SUMO route file")
}

/// Same as [`read_routes`], for callers that already have the `.rou.xml` bytes
/// in hand (an in-memory buffer, a decompressed stream, ...) rather than a
/// file on disk.
///
/// # Errors
///
/// Same as [`read_routes`], minus the failure to open a file.
pub fn read_routes_from(source: impl BufRead) -> Result<Routes> {
    read_document::<schema::RoutesType, _, _>(source, "routes", "SUMO route file")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::domain::{Depart, RouteId, VehicleId, VehicleTypeId};

    /// A vehicle type, a route, and a vehicle running it — the 3 building
    /// blocks this crate models (see the `domain` module docs for what's
    /// left out).
    const SAMPLE_ROUTES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<routes>
    <vType id="car" vClass="passenger" length="5.00" maxSpeed="50.00"/>
    <route id="r0" edges="e0 e1 e2"/>
    <vehicle id="v0" type="car" route="r0" depart="0.00"/>
</routes>
"#;

    #[test]
    fn reads_a_minimal_route_file_end_to_end() {
        let routes = read_routes_from(SAMPLE_ROUTES.as_bytes()).unwrap();

        assert_eq!(routes.vehicle_types.len(), 1);
        assert_eq!(routes.vehicle_types[0].id, VehicleTypeId("car".into()));

        assert_eq!(routes.routes.len(), 1);
        assert_eq!(routes.routes[0].id, RouteId("r0".into()));
        assert_eq!(routes.routes[0].edges.as_ref().unwrap().len(), 3);

        assert_eq!(routes.vehicles.len(), 1);
        assert_eq!(routes.vehicles[0].id, VehicleId("v0".into()));
        assert_eq!(
            routes.vehicles[0].vehicle_type,
            Some(VehicleTypeId("car".into()))
        );
        assert_eq!(routes.vehicles[0].route, Some(RouteId("r0".into())));
        assert!(matches!(routes.vehicles[0].depart, Depart::Time(_)));
    }

    #[test]
    fn rejects_unrelated_root_elements() {
        // `routesType`'s content is entirely optional (every element in its
        // `xsd:choice` has `minOccurs="0"`), so unlike `net`'s `NetType`
        // (which requires `<location>`), no required field would make an
        // unrelated document fail — it would parse as an empty `Routes`.
        // The root-name check in `read_routes_from` is the only thing
        // standing between a typo'd input and a silent empty result.
        let error = read_routes_from(b"<not-routes/>".as_slice()).unwrap_err();
        assert!(
            error.to_string().contains("<not-routes>"),
            "error should name the offending root element, got: {error}"
        );
    }

    #[test]
    fn accepts_an_empty_but_correctly_rooted_route_file() {
        let routes = read_routes_from(b"<routes/>".as_slice()).unwrap();
        assert_eq!(routes, Routes::default());
    }

    #[test]
    fn rejects_a_vehicle_type_missing_its_required_id() {
        assert!(
            read_routes_from(br#"<routes><vType vClass="passenger"/></routes>"#.as_slice())
                .is_err()
        );
    }
}
