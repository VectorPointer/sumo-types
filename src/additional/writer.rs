//! Writing an [`Additional`] to a `.add.xml` file: converts layer 2 into
//! layer 1 (the private `schema` module) via [`super::schema_writer`] and
//! serializes it in one step, mirroring `additional`'s own `reader`.

use super::domain::Additional;
use crate::Result;
use crate::schema;
use crate::xml::{write_document, write_document_at};
use std::io::Write;
use std::path::Path;

/// Writes `additional` to the `.add.xml` file at `path`, creating or
/// truncating it.
///
/// ```no_run
/// use sumo_types::additional::domain::{Additional, DetectorId, E1Detector, LanePosition, LaneRef};
/// use sumo_types::uom::si::f64::Length;
/// use sumo_types::uom::si::length::meter;
/// use std::path::Path;
///
/// let additional = Additional {
///     induction_loops: vec![E1Detector {
///         id: DetectorId("loop0".into()),
///         lane: LaneRef("e0_0".into()),
///         position: LanePosition::FromStart(Length::new::<meter>(12.5)),
///         file: "out/loop0.xml".into(),
///         period: None,
///         length: None,
///         name: None,
///         friendly_position: None,
///     }],
///     ..Additional::default()
/// };
/// sumo_types::additional::write_additional(Path::new("city.add.xml"), &additional)?;
/// # Ok::<(), sumo_types::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if `path` can't be created, or if writing to it fails.
pub fn write_additional(path: &Path, additional: &Additional) -> Result<()> {
    write_document_at::<schema::AdditionalType, _>(
        additional,
        "additional",
        "SUMO additional file",
        path,
    )
}

/// Same as [`write_additional`], for callers that want the `.add.xml` bytes
/// somewhere other than a file on disk (an in-memory buffer, a socket, ...).
///
/// # Errors
///
/// Same as [`write_additional`], minus the failure to create a file.
pub fn write_additional_to(additional: &Additional, sink: impl Write) -> Result<()> {
    write_document::<schema::AdditionalType, _, _>(
        additional,
        "additional",
        "SUMO additional file",
        sink,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::additional::domain::{
        DetectorGate, DetectorId, E1Detector, E2Detector, E3Detector, LaneCoverage, LanePosition,
        LaneRef, Point,
    };
    use crate::additional::reader::read_additional_from;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn sample_additional() -> Additional {
        Additional {
            induction_loops: vec![E1Detector {
                id: DetectorId("loop0".into()),
                lane: LaneRef("e0_0".into()),
                position: LanePosition::FromStart(Length::new::<meter>(12.5)),
                file: "out/loop0.xml".into(),
                period: None,
                length: None,
                name: None,
                friendly_position: None,
            }],
            lane_area_detectors: vec![E2Detector {
                id: DetectorId("area0".into()),
                coverage: Some(LaneCoverage::SingleLane(LaneRef("e0_0".into()))),
                position: None,
                end_position: Some(LanePosition::FromEnd(Length::new::<meter>(5.0))),
                length: None,
                file: "out/area0.xml".into(),
                period: None,
                name: None,
                friendly_position: None,
                speed_threshold: None,
                time_threshold: None,
                jam_threshold: None,
            }],
            entry_exit_detectors: vec![E3Detector {
                id: DetectorId("zone0".into()),
                entries: vec![DetectorGate {
                    lane: LaneRef("e0_0".into()),
                    position: LanePosition::FromStart(Length::new::<meter>(0.0)),
                    friendly_position: None,
                }],
                exits: vec![DetectorGate {
                    lane: LaneRef("e1_0".into()),
                    position: LanePosition::FromStart(Length::new::<meter>(100.0)),
                    friendly_position: Some(true),
                }],
                file: "out/zone0.xml".into(),
                period: None,
                name: None,
                icon_position: Some(Point {
                    x: 42.0,
                    y: 7.0,
                    z: 0.0,
                }),
                speed_threshold: None,
                time_threshold: None,
                open_entry: None,
            }],
        }
    }

    #[test]
    fn writes_a_document_a_correctly_rooted_reader_accepts() {
        let mut buf = Vec::new();
        write_additional_to(&sample_additional(), &mut buf).unwrap();

        let xml = String::from_utf8(buf).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<additional"));

        let read_back = read_additional_from(xml.as_bytes()).unwrap();
        assert_eq!(read_back, sample_additional());
    }
}
