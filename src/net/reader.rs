//! Reading a `.net.xml` file into a [`Network`]: deserializes the input into
//! layer 1 (the private `schema` module) and converts it to layer 2
//! ([`crate::domain`]) in one step, so consumers never have to name a
//! generated schema type or depend on `xsd-parser-types` themselves.

use super::domain::Network;
use crate::schema;
use crate::xml::{RootRecordingReader, ensure_root_is};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use xsd_parser_types::quick_xml::{DeserializeSync, IoReader};

/// Reads and deserializes the `.net.xml` file at `path` into a [`Network`].
///
/// ```no_run
/// let network = sumo_types::read_network(std::path::Path::new("city.net.xml"))?;
/// println!("{} edges", network.edges.len());
/// # Ok::<(), anyhow::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if `path` can't be opened, if the document isn't
/// well-formed XML rooted at `<net>`, or if any of its attributes can't be
/// interpreted (a malformed position, a boundary without 4 components, a
/// non-finite coordinate, ...).
pub fn read_network(path: &Path) -> Result<Network> {
    let input_file = File::open(path)
        .with_context(|| format!("Could not open input file: {}", path.display()))?;

    read_network_from(BufReader::new(input_file))
        .with_context(|| format!("invalid SUMO network in {}", path.display()))
}

/// Same as [`read_network`], for callers that already have the `.net.xml`
/// bytes in hand (an in-memory buffer, a decompressed stream, ...) rather
/// than a file on disk.
///
/// # Errors
///
/// Same as [`read_network`], minus the failure to open a file.
pub fn read_network_from(source: impl BufRead) -> Result<Network> {
    let mut reader = RootRecordingReader::new(IoReader::new(source));

    let net = schema::NetType::deserialize(&mut reader)
        .map_err(|error| anyhow::anyhow!("failed to parse SUMO network: {error}"))?;

    // After deserializing, not before: the root name is only known once the
    // reader has produced its first element event.
    ensure_root_is(&reader, "net", "SUMO network")?;

    Network::try_from(net)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EdgeId, JunctionKind, LaneId, Projection};

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
        assert_eq!(network.location.projection, Projection::None);
    }

    #[test]
    fn rejects_input_that_is_not_a_sumo_network() {
        assert!(read_network_from(b"<not-a-net/>".as_slice()).is_err());
    }

    #[test]
    fn rejects_a_net_shaped_document_under_the_wrong_root() {
        // xsd-parser generates deserializers for XSD *types*, not elements,
        // so without the root-name check in `read_network_from` this parses
        // happily into an empty `Network`.
        let disguised = SAMPLE_NET
            .replace("<net ", "<additional ")
            .replace("</net>", "</additional>");

        let error = read_network_from(disguised.as_bytes()).unwrap_err();
        assert!(
            error.to_string().contains("<additional>"),
            "error should name the offending root element, got: {error}"
        );
    }

    #[test]
    fn rejects_non_finite_coordinates() {
        let poisoned = SAMPLE_NET.replace(r#"netOffset="0.00,0.00""#, r#"netOffset="NaN,0.00""#);
        assert!(read_network_from(poisoned.as_bytes()).is_err());

        let poisoned = SAMPLE_NET.replace(r#"x="100.00""#, r#"x="inf""#);
        assert!(read_network_from(poisoned.as_bytes()).is_err());
    }
}
