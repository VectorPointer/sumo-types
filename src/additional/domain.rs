//! Layer 2 for `additional`: this crate's own domain types for `.add.xml`.
//!
//! Independent of SUMO/XSD, same rationale as `net`'s own `crate::domain`
//! (see the crate docs) — and independent of the other formats' domain
//! types, by design: `additional` doesn't require `net` or `routes` to be
//! enabled, so it can't borrow their types and defines its own [`LaneRef`]
//! instead.
//!
//! Those cross-format names are code spans, not intra-doc links: `net` may
//! not be enabled, and a link to a feature-gated item is a broken link in
//! every build that leaves that feature off.
//!
//! Only the detector definitions are modelled: [`E1Detector`] (induction
//! loop), [`E2Detector`] (lane area) and [`E3Detector`] (multi-entry /
//! multi-exit). `additionalType` allows about forty more element kinds —
//! `busStop`, `chargingStation`, `parkingArea`, `rerouter`, `calibrator`,
//! `variableSpeedSign`, `WAUT`, `poly`, `poi`, the whole of `routesType`
//! again, ... — none of which are mapped; [`Additional`] silently drops
//! them (see [`crate::additional::schema_mapper`]).

use derive_more::{AsRef, Display, From};
use uom::si::f64::{Length, Time, Velocity};

/// Identifier of a detector. A distinct type from a bare `String`, same
/// rationale as `net`'s `crate::domain::EdgeId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, AsRef)]
pub struct DetectorId(#[as_ref(forward)] pub String);

/// A point in the network's coordinate system, re-exported at this format's
/// own path. This *is* `net`'s `crate::domain::Point`, not a lookalike: the
/// two used to be separate byte-identical structs only because `additional`
/// cannot name a type from `net`, which may not be enabled, and both are now
/// the one definition in a module that is always compiled. Unlike
/// [`LaneRef`] below, nothing was gained by keeping them apart — see the
/// type's own docs.
pub use crate::sumo::Point;

/// Identifier of a lane a detector sits on, in the `.net.xml` this
/// `.add.xml` accompanies. Deliberately not `net`'s `crate::domain::LaneId`
/// — see the module docs for why. The two carry the same string ids but
/// aren't the same Rust type, so they can't be mixed up by accident.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, AsRef)]
pub struct LaneRef(#[as_ref(forward)] pub String);

/// A position along a lane.
///
/// SUMO encodes the two cases in the sign of one number: a negative `pos`
/// means "measured back from the lane's end". That is spelled out as two
/// variants rather than left as a signed [`Length`] because the difference
/// is not cosmetic — reading a `-12.5` as a distance from the start puts a
/// detector at the wrong end of the road, and nothing about the number
/// itself says so.
///
/// Neither variant is resolved to the other here: doing that needs the
/// lane's length, which lives in the `.net.xml` this format deliberately
/// doesn't require to be loaded alongside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LanePosition {
    /// Distance from the lane's start, SUMO's non-negative `pos`.
    FromStart(Length),
    /// Distance back from the lane's end, i.e. SUMO's negative `pos` with
    /// the sign already taken off: `pos="-12.5"` is `FromEnd(12.5 m)`.
    FromEnd(Length),
}

/// One entry or exit point of an [`E3Detector`], mirroring SUMO's
/// `detEntryExitType`.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectorGate {
    pub lane: LaneRef,
    pub position: LanePosition,
    /// SUMO's `friendlyPos`: whether a position outside the lane is clamped
    /// into range instead of being an error. `None` when the attribute is
    /// absent, rather than resolved to SUMO's default, so a round-trip can
    /// tell "unset" from "explicitly set".
    pub friendly_position: Option<bool>,
}

/// An induction loop (SUMO's `e1Detector`, also spelled `inductionLoop`): a
/// point detector at a fixed position on a single lane.
#[derive(Debug, Clone, PartialEq)]
pub struct E1Detector {
    pub id: DetectorId,
    pub lane: LaneRef,
    pub position: LanePosition,
    /// Where the detector writes its output, as given — a path relative to
    /// the `.add.xml` itself, per how SUMO resolves it.
    pub file: String,
    pub period: Option<Time>,
    pub length: Option<Length>,
    pub name: Option<String>,
    pub friendly_position: Option<bool>,
}

/// What an [`E2Detector`] is stretched over: SUMO's mutually exclusive
/// `lane` and `lanes` attributes, as one value that can only hold one of
/// them. Setting both is an error SUMO rejects, and this makes it
/// unrepresentable rather than something each consumer has to check.
#[derive(Debug, Clone, PartialEq)]
pub enum LaneCoverage {
    /// A stretch of one lane (`lane`).
    SingleLane(LaneRef),
    /// A chain of consecutive lanes (`lanes`).
    LaneChain(Vec<LaneRef>),
}

/// A lane area detector (SUMO's `e2Detector`, also spelled
/// `laneAreaDetector`): covers a stretch of one lane, or a chain of lanes.
#[derive(Debug, Clone, PartialEq)]
pub struct E2Detector {
    pub id: DetectorId,
    /// Which lanes the detector covers. `None` when the document set
    /// neither `lane` nor `lanes`, which `additional_file.xsd` permits
    /// (both are `use="optional"`) even though SUMO itself rejects it.
    pub coverage: Option<LaneCoverage>,
    pub position: Option<LanePosition>,
    pub end_position: Option<LanePosition>,
    pub length: Option<Length>,
    pub file: String,
    pub period: Option<Time>,
    pub name: Option<String>,
    pub friendly_position: Option<bool>,
    /// `speedThreshold`: below this speed a vehicle counts as halting.
    pub speed_threshold: Option<Velocity>,
    /// `timeThreshold`: how long a vehicle must stay under
    /// [`Self::speed_threshold`] before it counts as halting.
    pub time_threshold: Option<Time>,
    /// `jamThreshold`: how close the next halting vehicle has to be for the
    /// two to count as one jam. A distance, not a speed — it is a separate
    /// attribute from [`Self::speed_threshold`], which an earlier version of
    /// this file conflated.
    pub jam_threshold: Option<Length>,
}

/// A multi-entry / multi-exit detector (SUMO's `e3Detector`, also spelled
/// `entryExitDetector`): an area of the network delimited by
/// [`DetectorGate`]s, measuring whatever is between them.
#[derive(Debug, Clone, PartialEq)]
pub struct E3Detector {
    pub id: DetectorId,
    pub entries: Vec<DetectorGate>,
    pub exits: Vec<DetectorGate>,
    pub file: String,
    pub period: Option<Time>,
    pub name: Option<String>,
    /// SUMO's `pos`: where editors like netedit draw the detector's icon.
    /// Purely cosmetic — SUMO uses the gates, not this, for detection.
    ///
    /// `additional_file.xsd` types this attribute as a bare `xsd:string`
    /// rather than one of SUMO's position types, but every writer emits
    /// `"x,y"`, so it is parsed like any other position and a value that
    /// isn't one is an error — same treatment the rest of the crate gives
    /// a malformed coordinate.
    pub icon_position: Option<Point>,
    /// `speedThreshold`: below this speed a vehicle counts as halting.
    pub speed_threshold: Option<Velocity>,
    /// `timeThreshold`: how long a vehicle must stay under
    /// [`Self::speed_threshold`] before it counts as halting.
    pub time_threshold: Option<Time>,
    /// `openEntry`: whether vehicles already inside the area at the start
    /// are counted, rather than only those seen crossing an entry gate.
    pub open_entry: Option<bool>,
}

/// A `.add.xml` file's content, restricted to what this crate models — see
/// the module docs for what's dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Additional {
    pub induction_loops: Vec<E1Detector>,
    pub lane_area_detectors: Vec<E2Detector>,
    pub entry_exit_detectors: Vec<E3Detector>,
}
