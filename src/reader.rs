//! Reading a `.net.xml` file into a [`Network`]: deserializes the input into
//! layer 1 ([`crate::schema`]) and converts it to layer 2
//! ([`crate::domain`]) in one step, so consumers never have to name a
//! generated schema type or depend on `xsd-parser-types` themselves.

use crate::domain::Network;
use crate::schema;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use xsd_parser_types::quick_xml::{DeserializeSync, IoReader};

/// Reads and deserializes the `.net.xml` file at `path` into a [`Network`].
pub fn read_network(path: &Path) -> Result<Network> {
    let input_file =
        File::open(path).with_context(|| format!("Could not open input file: {path:?}"))?;

    read_network_from(BufReader::new(input_file))
        .with_context(|| format!("invalid SUMO network in {path:?}"))
}

/// Same as [`read_network`], for callers that already have the `.net.xml`
/// bytes in hand (an in-memory buffer, a decompressed stream, ...) rather
/// than a file on disk.
pub fn read_network_from(source: impl BufRead) -> Result<Network> {
    let mut reader = IoReader::new(source);

    let net = schema::NetType::deserialize(&mut reader)
        .map_err(|error| anyhow::anyhow!("failed to parse SUMO network: {error}"))?;

    Network::try_from(net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EdgeId, JunctionKind, LaneId};

    /// Minimal `.net.xml`: one edge with one lane, joining two junctions,
    /// one of them traffic-light controlled with a two-phase program.
    ///
    /// Element order matters — `net_file.xsd` declares the children of
    /// `<net>` as an `xsd:sequence`, so this mirrors the order netconvert
    /// itself emits (location, type, edge, tlLogic, junction, connection).
    const SAMPLE_NET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<net version="1.20" junctionCornerDetail="5" limitTurnSpeed="5.50">
    <location netOffset="0.00,0.00" convBoundary="0.00,0.00,100.00,0.00" origBoundary="-1.00,-1.00,1.00,1.00" projParameter="!"/>
    <edge id="e0" from="j0" to="j1" priority="1">
        <lane id="e0_0" index="0" speed="13.89" length="100.00" shape="0.00,0.00 100.00,0.00"/>
    </edge>
    <tlLogic id="j1" type="static" programID="0" offset="0">
        <phase duration="42" state="G"/>
        <phase duration="3"  state="r"/>
    </tlLogic>
    <junction id="j0" type="dead_end" x="0.00" y="0.00" incLanes="" intLanes="" shape="0.00,0.00"/>
    <junction id="j1" type="traffic_light" x="100.00" y="0.00" incLanes="e0_0" intLanes="" shape="100.00,0.00"/>
    <connection from="e0" to="e0" fromLane="0" toLane="0" dir="s" state="M"/>
</net>
"#;

    #[test]
    fn reads_a_minimal_network_end_to_end() {
        let network = read_network_from(SAMPLE_NET.as_bytes()).unwrap();

        assert_eq!(network.edges.len(), 1);
        assert_eq!(network.edges[0].id, EdgeId("e0".into()));
        assert_eq!(network.edges[0].lanes[0].id, LaneId("e0_0".into()));

        assert_eq!(network.junctions.len(), 2);
        assert_eq!(network.junctions[1].kind, JunctionKind::TrafficLight);
        assert_eq!(
            network.junctions[1].incoming_lanes,
            vec![LaneId("e0_0".into())]
        );

        assert_eq!(network.traffic_light_programs.len(), 1);
        assert_eq!(network.traffic_light_programs[0].phases, vec!["G", "r"]);

        assert_eq!(network.connections.len(), 1);
        assert!(network.location.is_some());
    }

    #[test]
    fn rejects_input_that_is_not_a_sumo_network() {
        assert!(read_network_from(b"<not-a-net/>".as_slice()).is_err());
    }
}
