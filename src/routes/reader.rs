//! Reading a `.rou.xml` file into [`Routes`]: deserializes the input into
//! layer 1 ([`crate::schema`]) and converts it to layer 2
//! ([`crate::routes::domain`]) in one step, mirroring
//! [`crate::net::reader`].

use super::domain::Routes;
use crate::schema;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use xsd_parser_types::quick_xml::{DeserializeSync, IoReader};

/// Reads and deserializes the `.rou.xml` file at `path` into [`Routes`].
pub fn read_routes(path: &Path) -> Result<Routes> {
    let input_file =
        File::open(path).with_context(|| format!("Could not open input file: {path:?}"))?;

    read_routes_from(BufReader::new(input_file))
        .with_context(|| format!("invalid SUMO route file in {path:?}"))
}

/// Same as [`read_routes`], for callers that already have the `.rou.xml`
/// bytes in hand (an in-memory buffer, a decompressed stream, ...) rather
/// than a file on disk.
pub fn read_routes_from(source: impl BufRead) -> Result<Routes> {
    let mut reader = IoReader::new(source);

    let routes = schema::RoutesType::deserialize(&mut reader)
        .map_err(|error| anyhow::anyhow!("failed to parse SUMO route file: {error}"))?;

    Routes::try_from(routes)
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
        assert_eq!(routes.vehicles[0].vehicle_type, Some(VehicleTypeId("car".into())));
        assert_eq!(routes.vehicles[0].route, Some(RouteId("r0".into())));
        assert!(matches!(routes.vehicles[0].depart, Depart::Time(_)));
    }

    #[test]
    fn ignores_unrelated_root_elements() {
        // `routesType`'s content is entirely optional (every element in its
        // `xsd:choice` has `minOccurs="0"`), so unlike `net`'s `NetType`
        // (which requires `<location>`), there's no required field for a
        // completely unrelated document to fail on — it just parses as
        // empty. This documents that permissiveness rather than asserting
        // a rejection that wouldn't actually happen.
        let routes = read_routes_from(b"<not-routes/>".as_slice()).unwrap();
        assert_eq!(routes, Routes::default());
    }

    #[test]
    fn rejects_a_vehicle_type_missing_its_required_id() {
        assert!(read_routes_from(br#"<routes><vType vClass="passenger"/></routes>"#.as_slice()).is_err());
    }
}
