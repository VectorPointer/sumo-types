//! Reading a `.add.xml` file into [`Additional`]: deserializes the input
//! into layer 1 (the private `schema` module) and converts it to layer 2
//! ([`crate::additional::domain`]) in one step, mirroring the other
//! formats' readers.

use super::domain::Additional;
use crate::schema;
use crate::xml::{RootRecordingReader, ensure_root_is};
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use xsd_parser_types::quick_xml::{DeserializeSync, IoReader};

/// Reads and deserializes the `.add.xml` file at `path` into [`Additional`].
///
/// # Errors
///
/// Returns an error if `path` can't be opened, if the document isn't
/// well-formed XML rooted at `<additional>`, or if any of its attributes
/// can't be interpreted (a malformed or non-finite position, a boolean with
/// no binary meaning, ...).
pub fn read_additional(path: &Path) -> Result<Additional> {
    let input_file = File::open(path)
        .with_context(|| format!("Could not open input file: {}", path.display()))?;

    read_additional_from(BufReader::new(input_file))
        .with_context(|| format!("invalid SUMO additional file in {}", path.display()))
}

/// Same as [`read_additional`], for callers that already have the
/// `.add.xml` bytes in hand (an in-memory buffer, a decompressed stream,
/// ...) rather than a file on disk.
///
/// # Errors
///
/// Same as [`read_additional`], minus the failure to open a file.
pub fn read_additional_from(source: impl BufRead) -> Result<Additional> {
    let mut reader = RootRecordingReader::new(IoReader::new(source));

    let additional = schema::AdditionalType::deserialize(&mut reader)
        .map_err(|error| anyhow::anyhow!("failed to parse SUMO additional file: {error}"))?;

    // `additionalType`'s content is entirely optional, so without this check
    // any document at all would parse as an empty `Additional` (see the
    // tests).
    ensure_root_is(&reader, "additional", "SUMO additional file")?;

    Additional::try_from(additional)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::additional::domain::{DetectorId, LaneRef};
    use uom::si::length::meter;
    use uom::si::time::second;

    /// One of each detector kind, written the way SUMO's own tools emit
    /// them (`netedit` writes `e1Detector`/`e2Detector`/`e3Detector`).
    const SAMPLE_ADDITIONAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<additional>
    <e1Detector id="loop0" lane="e0_0" pos="12.50" period="60.00" file="out/loop0.xml"/>
    <e2Detector id="area0" lane="e0_0" pos="0.00" endPos="50.00" file="out/area0.xml"/>
    <e3Detector id="zone0" file="out/zone0.xml" pos="42.00,7.00">
        <detEntry lane="e0_0" pos="0.00"/>
        <detEntry lane="e1_0" pos="0.00"/>
        <detExit lane="e0_0" pos="100.00"/>
    </e3Detector>
</additional>
"#;

    #[test]
    fn reads_one_of_each_detector_kind_end_to_end() {
        let additional = read_additional_from(SAMPLE_ADDITIONAL.as_bytes()).unwrap();

        assert_eq!(additional.induction_loops.len(), 1);
        let loop0 = &additional.induction_loops[0];
        assert_eq!(loop0.id, DetectorId("loop0".into()));
        assert_eq!(loop0.lane, LaneRef("e0_0".into()));
        assert_eq!(loop0.position.0.get::<meter>(), 12.5);
        assert_eq!(loop0.period.unwrap().get::<second>(), 60.0);

        assert_eq!(additional.lane_area_detectors.len(), 1);
        let area0 = &additional.lane_area_detectors[0];
        assert_eq!(area0.lane, Some(LaneRef("e0_0".into())));
        assert_eq!(area0.end_position.unwrap().0.get::<meter>(), 50.0);

        assert_eq!(additional.entry_exit_detectors.len(), 1);
        let zone0 = &additional.entry_exit_detectors[0];
        assert_eq!(zone0.entries.len(), 2);
        assert_eq!(zone0.exits.len(), 1);
        assert_eq!(zone0.exits[0].position.0.get::<meter>(), 100.0);
        assert_eq!(zone0.icon_position.as_deref(), Some("42.00,7.00"));
    }

    #[test]
    fn treats_sumos_alternative_element_names_as_the_same_detector() {
        // `inductionLoop`/`laneAreaDetector`/`entryExitDetector` are the
        // same XSD types as `e1Detector`/`e2Detector`/`e3Detector` under a
        // second element name, and must not land in different buckets.
        let xml = r#"<additional>
            <inductionLoop id="loop0" lane="e0_0" pos="1.00" file="a.xml"/>
            <laneAreaDetector id="area0" lane="e0_0" file="b.xml"/>
            <entryExitDetector id="zone0" file="c.xml"><detEntry lane="e0_0" pos="0.00"/></entryExitDetector>
        </additional>"#;

        let additional = read_additional_from(xml.as_bytes()).unwrap();

        assert_eq!(additional.induction_loops.len(), 1);
        assert_eq!(additional.lane_area_detectors.len(), 1);
        assert_eq!(additional.entry_exit_detectors.len(), 1);
    }

    #[test]
    fn ignores_element_kinds_this_crate_does_not_model() {
        let xml = r#"<additional>
            <busStop id="stop0" lane="e0_0" startPos="0.00" endPos="20.00"/>
            <e1Detector id="loop0" lane="e0_0" pos="1.00" file="a.xml"/>
        </additional>"#;

        let additional = read_additional_from(xml.as_bytes()).unwrap();

        assert_eq!(additional.induction_loops.len(), 1);
        assert!(additional.lane_area_detectors.is_empty());
    }

    #[test]
    fn rejects_unrelated_root_elements() {
        // `additionalType`'s content is entirely optional, so nothing else
        // would make an unrelated document fail — it would parse as an
        // empty `Additional`.
        let error = read_additional_from(b"<not-additional/>".as_slice()).unwrap_err();
        assert!(
            error.to_string().contains("<not-additional>"),
            "error should name the offending root element, got: {error}"
        );
    }

    #[test]
    fn accepts_an_empty_but_correctly_rooted_file() {
        let additional = read_additional_from(b"<additional/>".as_slice()).unwrap();
        assert_eq!(additional, Additional::default());
    }

    #[test]
    fn rejects_a_non_finite_position() {
        let xml =
            r#"<additional><e1Detector id="l" lane="e0_0" pos="NaN" file="a.xml"/></additional>"#;
        assert!(read_additional_from(xml.as_bytes()).is_err());
    }
}
