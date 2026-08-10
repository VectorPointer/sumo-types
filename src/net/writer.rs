//! Writing a [`Network`] to a `.net.xml` file: converts layer 2 into layer 1
//! (the private `schema` module) via [`super::schema_writer`] and
//! serializes it in one step, mirroring `net`'s own `reader`.

use super::domain::Network;
use crate::Result;
use crate::schema;
use crate::xml::{write_document, write_document_at};
use std::io::Write;
use std::path::Path;

/// Writes `network` to the `.net.xml` file at `path`, creating or
/// truncating it.
///
/// ```no_run
/// let network = sumo_types::read_network(std::path::Path::new("city.net.xml"))?;
/// sumo_types::write_network(std::path::Path::new("city-copy.net.xml"), &network)?;
/// # Ok::<(), sumo_types::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if `path` can't be created, or if writing to it fails.
pub fn write_network(path: &Path, network: &Network) -> Result<()> {
    write_document_at::<schema::NetType, _>(network, "net", "SUMO network", path)
}

/// Same as [`write_network`], for callers that want the `.net.xml` bytes
/// somewhere other than a file on disk (an in-memory buffer, a socket, ...).
///
/// # Errors
///
/// Same as [`write_network`], minus the failure to create a file.
pub fn write_network_to(network: &Network, sink: impl Write) -> Result<()> {
    write_document::<schema::NetType, _, _>(network, "net", "SUMO network", sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        Boundary, Edge, EdgeFunction, EdgeId, Junction, JunctionId, JunctionKind, Lane, LaneId,
        LaneIndex, Location, Phase, Point, Projection, Shape, TrafficLightId, TrafficLightKind,
        TrafficLightOffset, TrafficLightProgram,
    };
    use crate::net::reader::read_network_from;
    use uom::si::f64::{Length, Time, Velocity};
    use uom::si::length::meter;
    use uom::si::time::second;
    use uom::si::velocity::meter_per_second;

    /// `speed`/`length`/`width` use values that are exact in `f32`
    /// (`13.75`, `3.25`) rather than realistic-looking ones (`13.89`,
    /// `3.2`): `.net.xml`'s `speed`/`width` are real `xsd:float`s, so
    /// `schema_writer` narrows this crate's `f64` `uom` quantities down to
    /// `f32` on the way out — the same narrowing `schema_mapper` already
    /// does widening back up on the way in (see
    /// `converts_lane_speed_and_length_with_real_units` in
    /// `net/schema_mapper.rs`, which tolerance-compares for the same
    /// reason). Picking `f32`-exact literals here keeps this test an exact
    /// `assert_eq!` on the whole `Network` instead of a field-by-field
    /// tolerance check.
    fn sample_network() -> Network {
        Network {
            location: Location {
                net_offset: Point {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                converted_boundary: Boundary {
                    min: (0.0, 0.0),
                    max: (100.0, 0.0),
                },
                original_boundary: Boundary {
                    min: (-1.0, -1.0),
                    max: (1.0, 1.0),
                },
                projection: Projection::None,
            },
            edges: vec![Edge {
                id: EdgeId("e0".into()),
                function: EdgeFunction::Normal,
                from: Some(JunctionId("j0".into())),
                to: Some(JunctionId("j1".into())),
                name: None,
                priority: None,
                length: None,
                shape: None,
                spread_type: None,
                lanes: vec![Lane {
                    id: LaneId("e0_0".into()),
                    index: LaneIndex(0),
                    speed: Velocity::new::<meter_per_second>(13.75),
                    length: Length::new::<meter>(100.0),
                    width: Length::new::<meter>(3.25),
                    end_offset: Length::new::<meter>(0.0),
                    shape: Shape(vec![
                        Point {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        Point {
                            x: 100.0,
                            y: 0.0,
                            z: 0.0,
                        },
                    ]),
                    allow: vec![],
                    disallow: vec![],
                }],
            }],
            junctions: vec![
                Junction {
                    id: JunctionId("j0".into()),
                    position: Point {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    kind: JunctionKind::DeadEnd,
                    incoming_lanes: vec![],
                    internal_lanes: vec![],
                    shape: None,
                    name: None,
                },
                Junction {
                    id: JunctionId("j1".into()),
                    position: Point {
                        x: 100.0,
                        y: 0.0,
                        z: 0.0,
                    },
                    kind: JunctionKind::TrafficLight,
                    incoming_lanes: vec![LaneId("e0_0".into())],
                    internal_lanes: vec![],
                    shape: None,
                    name: None,
                },
            ],
            connections: vec![],
            roundabouts: vec![],
            traffic_light_programs: vec![TrafficLightProgram {
                id: TrafficLightId("j1".into()),
                program_id: "0".into(),
                kind: Some(TrafficLightKind::Static),
                offset: Some(TrafficLightOffset::Fixed(Time::new::<second>(0.0))),
                phases: vec![
                    Phase {
                        duration: Time::new::<second>(42.0),
                        state: "G".into(),
                    },
                    Phase {
                        duration: Time::new::<second>(3.0),
                        state: "r".into(),
                    },
                ],
            }],
        }
    }

    #[test]
    fn writes_a_document_a_correctly_rooted_reader_accepts() {
        let mut buf = Vec::new();
        write_network_to(&sample_network(), &mut buf).unwrap();

        let xml = String::from_utf8(buf).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<net"));

        let read_back = read_network_from(xml.as_bytes()).unwrap();
        assert_eq!(read_back, sample_network());
    }
}
