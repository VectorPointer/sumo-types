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

/// Identifier of a lane a detector sits on, in the `.net.xml` this
/// `.add.xml` accompanies. Deliberately not `net`'s `crate::domain::LaneId`
/// — see the module docs for why. The two carry the same string ids but
/// aren't the same Rust type, so they can't be mixed up by accident.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, AsRef)]
pub struct LaneRef(#[as_ref(forward)] pub String);

/// A position along a lane, measured from the lane's start.
///
/// SUMO also accepts a negative value, meaning "measured back from the
/// lane's end"; it is kept as given rather than resolved here, because
/// resolving it needs the lane's length, which lives in the `.net.xml` this
/// crate deliberately doesn't require to be loaded alongside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LanePosition(pub Length);

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

/// A lane area detector (SUMO's `e2Detector`, also spelled
/// `laneAreaDetector`): covers a stretch of one lane, or a chain of lanes.
#[derive(Debug, Clone, PartialEq)]
pub struct E2Detector {
    pub id: DetectorId,
    /// Set when the detector covers a single lane (`lane`); `None` when it
    /// spans a chain, in which case [`Self::lanes`] is populated instead.
    /// SUMO's schema allows both attributes to be absent, so neither field
    /// is required here.
    pub lane: Option<LaneRef>,
    /// The lane chain (`lanes`), when the detector spans several.
    pub lanes: Vec<LaneRef>,
    pub position: Option<LanePosition>,
    pub end_position: Option<LanePosition>,
    pub length: Option<Length>,
    pub file: String,
    pub period: Option<Time>,
    pub name: Option<String>,
    pub friendly_position: Option<bool>,
    /// `jamThreshold`: minimum speed below which a vehicle counts as
    /// jammed.
    pub speed_threshold: Option<Velocity>,
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
    /// SUMO's `pos`: where editors like netedit draw the detector's icon,
    /// as the raw `"x,y"` string. Purely cosmetic — SUMO uses the gates,
    /// not this, for detection. Kept unparsed because `net`'s `Point` isn't
    /// available here (see the module docs) and this format has no use for
    /// a parsed one.
    pub icon_position: Option<String>,
    pub speed_threshold: Option<Velocity>,
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
