//! Conversion from layer 2 ([`crate::additional::domain`]) to layer 1 (the
//! private `schema` module) — the inverse of `schema_mapper`, only compiled
//! under `write`.
//!
//! Every element kind `additionalType` allows beyond the three detectors
//! this crate models (`busStop`, `rerouter`, `calibrator`, ...) is simply
//! never emitted — [`Additional`] has nowhere to have kept one in the first
//! place, the same way `read_additional` has nowhere to have put one on the
//! way in. See the `domain` module docs for the full list.
//!
//! Conversions borrow (`TryFrom<&domain::X>`) rather than consume, same
//! rationale as `routes`' own `schema_writer`.

use super::domain::{
    Additional, DetectorGate, E1Detector, E2Detector, E3Detector, LaneCoverage, LanePosition,
};
use crate::schema;
use crate::sumo::{Point, format_bool_opt, format_finite, join_ids};
use crate::{Error, Result};
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Formats [`LanePosition`] back into SUMO's signed `pos` encoding — the
/// inverse of `schema_mapper`'s `lane_position`. `FromEnd`'s distance is
/// always non-negative (see that variant's own docs), so negating it can't
/// produce a double sign.
fn format_lane_position(value: &LanePosition) -> String {
    match value {
        LanePosition::FromStart(length) => format_finite(length.get::<meter>()),
        LanePosition::FromEnd(length) => format!("-{}", format_finite(length.get::<meter>())),
    }
}

impl From<&DetectorGate> for schema::DetEntryExitType {
    fn from(value: &DetectorGate) -> Self {
        schema::DetEntryExitType {
            lane: value.lane.0.clone(),
            pos: format_lane_position(&value.position),
            friendly_pos: format_bool_opt(value.friendly_position),
            content: schema::DetEntryExitTypeContent { param: Vec::new() },
        }
    }
}

impl From<&E1Detector> for schema::E1DetectorType {
    fn from(value: &E1Detector) -> Self {
        schema::E1DetectorType {
            id: value.id.0.clone(),
            lane: value.lane.0.clone(),
            pos: format_lane_position(&value.position),
            length: value.length.map(|l| l.get::<meter>() as f32),
            period: value.period.map(|p| p.get::<second>() as f32),
            freq: None,
            file: value.file.clone(),
            v_types: None,
            next_edges: None,
            detect_persons: None,
            name: value.name.clone(),
            friendly_pos: format_bool_opt(value.friendly_position),
            type_: None,
            content: schema::E1DetectorTypeContent { param: Vec::new() },
        }
    }
}

/// Splits [`E2Detector::coverage`] back into `e2Detector`'s mutually
/// exclusive `lane`/`lanes` attributes — the inverse of `schema_mapper`'s
/// `lane_coverage`, minus the validation: [`LaneCoverage`] already makes
/// "both set" and "`lanes` with `length`" unrepresentable, so there is
/// nothing left to reject on the way back out.
fn lane_coverage(value: &Option<LaneCoverage>) -> (Option<String>, Option<String>) {
    match value {
        Some(LaneCoverage::SingleLane(lane)) => (Some(lane.0.clone()), None),
        Some(LaneCoverage::LaneChain(lanes)) => (None, Some(join_ids(lanes))),
        None => (None, None),
    }
}

impl From<&E2Detector> for schema::E2DetectorType {
    fn from(value: &E2Detector) -> Self {
        let (lane, lanes) = lane_coverage(&value.coverage);

        schema::E2DetectorType {
            id: value.id.0.clone(),
            lane,
            lanes,
            file: value.file.clone(),
            pos: value.position.as_ref().map(format_lane_position),
            end_pos: value.end_position.as_ref().map(format_lane_position),
            length: value.length.map(|l| format_finite(l.get::<meter>())),
            period: value.period.map(|p| p.get::<second>() as f32),
            freq: None,
            tl: None,
            to: None,
            cont: None,
            time_threshold: value
                .time_threshold
                .map(|t| format_finite(t.get::<second>())),
            speed_threshold: value
                .speed_threshold
                .map(|s| format_finite(s.get::<meter_per_second>())),
            jam_threshold: value.jam_threshold.map(|j| format_finite(j.get::<meter>())),
            v_types: None,
            next_edges: None,
            detect_persons: None,
            name: value.name.clone(),
            friendly_pos: format_bool_opt(value.friendly_position),
            show: None,
            content: schema::E2DetectorTypeContent { param: Vec::new() },
        }
    }
}

impl From<&E3Detector> for schema::E3DetectorType {
    fn from(value: &E3Detector) -> Self {
        // `detEntry`/`detExit` each require at least one occurrence once
        // their branch of the choice is picked (`minOccurs="1"` in
        // `additional_file.xsd`), so an empty side is left out of
        // `det_gate_group` entirely rather than written as an empty group —
        // matching how `schema_mapper`'s read side only ever produces a
        // non-empty `Vec` for a branch that was actually present.
        let mut det_gate_group = Vec::new();
        if !value.entries.is_empty() {
            det_gate_group.push(schema::E3DetectorDetGateGroupType {
                content: schema::E3DetectorDetGateGroupTypeContent::detEntry(
                    value
                        .entries
                        .iter()
                        .map(schema::DetEntryExitType::from)
                        .collect(),
                ),
            });
        }
        if !value.exits.is_empty() {
            det_gate_group.push(schema::E3DetectorDetGateGroupType {
                content: schema::E3DetectorDetGateGroupTypeContent::detExit(
                    value
                        .exits
                        .iter()
                        .map(schema::DetEntryExitType::from)
                        .collect(),
                ),
            });
        }

        schema::E3DetectorType {
            id: value.id.0.clone(),
            period: value.period.map(|p| p.get::<second>() as f32),
            freq: None,
            file: value.file.clone(),
            time_threshold: value
                .time_threshold
                .map(|t| format_finite(t.get::<second>())),
            speed_threshold: value
                .speed_threshold
                .map(|s| s.get::<meter_per_second>() as f32),
            v_types: None,
            next_edges: None,
            detect_persons: None,
            open_entry: format_bool_opt(value.open_entry),
            expect_arrival: None,
            pos: value.icon_position.as_ref().map(Point::format),
            name: value.name.clone(),
            content: schema::E3DetectorTypeContent {
                param: Vec::new(),
                det_gate_group,
            },
        }
    }
}

impl TryFrom<&Additional> for schema::AdditionalType {
    type Error = Error;

    fn try_from(value: &Additional) -> Result<Self> {
        let induction_loops = value
            .induction_loops
            .iter()
            .map(schema::E1DetectorType::from)
            .map(Some)
            .map(schema::AdditionalTypeContent::e1Detector);

        let lane_area_detectors = value
            .lane_area_detectors
            .iter()
            .map(schema::E2DetectorType::from)
            .map(Some)
            .map(schema::AdditionalTypeContent::e2Detector);

        let entry_exit_detectors = value
            .entry_exit_detectors
            .iter()
            .map(schema::E3DetectorType::from)
            .map(Some)
            .map(schema::AdditionalTypeContent::e3Detector);

        Ok(schema::AdditionalType {
            content: induction_loops
                .chain(lane_area_detectors)
                .chain(entry_exit_detectors)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::additional::domain::{DetectorId, LaneRef};
    use uom::si::f64::Length;

    #[test]
    fn formats_lane_position_signs_match_which_end_they_measure_from() {
        assert_eq!(
            format_lane_position(&LanePosition::FromStart(Length::new::<meter>(12.5))),
            "12.5"
        );
        assert_eq!(
            format_lane_position(&LanePosition::FromEnd(Length::new::<meter>(12.5))),
            "-12.5"
        );
    }

    #[test]
    fn splits_lane_coverage_into_the_mutually_exclusive_attributes() {
        assert_eq!(
            lane_coverage(&Some(LaneCoverage::SingleLane(LaneRef("e0_0".into())))),
            (Some("e0_0".to_string()), None)
        );
        assert_eq!(
            lane_coverage(&Some(LaneCoverage::LaneChain(vec![
                LaneRef("e0_0".into()),
                LaneRef("e1_0".into())
            ]))),
            (None, Some("e0_0 e1_0".to_string()))
        );
        assert_eq!(lane_coverage(&None), (None, None));
    }

    #[test]
    fn omits_a_detector_gate_group_for_the_side_that_has_no_gates() {
        let detector = E3Detector {
            id: DetectorId("zone0".into()),
            entries: vec![DetectorGate {
                lane: LaneRef("e0_0".into()),
                position: LanePosition::FromStart(Length::new::<meter>(0.0)),
                friendly_position: None,
            }],
            exits: vec![],
            file: "out.xml".into(),
            period: None,
            name: None,
            icon_position: None,
            speed_threshold: None,
            time_threshold: None,
            open_entry: None,
        };

        let raw = schema::E3DetectorType::from(&detector);
        assert_eq!(raw.content.det_gate_group.len(), 1);
        assert!(matches!(
            raw.content.det_gate_group[0].content,
            schema::E3DetectorDetGateGroupTypeContent::detEntry(_)
        ));
    }
}
