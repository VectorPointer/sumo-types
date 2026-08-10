//! Layer 2 for `routes`: this crate's own domain types for `.rou.xml`.
//!
//! Independent of SUMO/XSD, same rationale as `net`'s own `crate::domain`
//! (see the crate docs) — and independent of `net`'s own domain types too,
//! by design: `routes` doesn't require `net` to be enabled, so it can't
//! borrow `crate::domain::EdgeId` and defines its own [`EdgeRef`] instead.
//!
//! Those cross-format names are code spans, not intra-doc links: `net` may
//! not be enabled, and a link to a feature-gated item is a broken link in
//! every build that leaves that feature off.
//!
//! Only the building blocks of a traffic demand file are modeled:
//! [`Route`], [`VehicleType`], and [`Vehicle`]. `routesType` also allows
//! `flow`, `trip`, `person`, `personFlow`, `container`, `containerFlow`,
//! `interval`, `include`, `vTypeDistribution`, and `routeDistribution`
//! elements, none of which are mapped yet — [`Routes`] silently drops them
//! (see [`crate::routes::schema_mapper`]).

use derive_more::{AsRef, Display, From};
use uom::si::f64::{Length, Time, Velocity};

/// Identifier of a [`Route`]. A distinct type from a bare `String`, same
/// rationale as `net`'s `crate::domain::EdgeId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, AsRef)]
pub struct RouteId(#[as_ref(forward)] pub String);

/// Identifier of a [`VehicleType`] (SUMO's confusingly-named `vType`
/// element — not to be confused with [`Vehicle`], an actual vehicle
/// instance; see that type's docs).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, AsRef)]
pub struct VehicleTypeId(#[as_ref(forward)] pub String);

/// Identifier of a [`Vehicle`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, AsRef)]
pub struct VehicleId(#[as_ref(forward)] pub String);

/// Identifier of an edge in the `.net.xml` a route file's [`Route::edges`]
/// refer to. Deliberately not `crate::domain::EdgeId` — see the module
/// docs for why. If both `net` and `routes` are enabled and describe the
/// same network, the two id newtypes carry the same string ids but aren't
/// the same Rust type, so they can't be mixed up by accident.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, AsRef)]
pub struct EdgeRef(#[as_ref(forward)] pub String);

/// SUMO's `colorType`: either explicit RGBA components or one of SUMO's
/// named colors. Kept as two variants rather than resolving named colors to
/// RGBA: `invisible` and `random` aren't fixed colors to begin with, and
/// this crate doesn't want to guess RGB values for the rest independently
/// of SUMO's own source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    /// Components parsed from `"r,g,b"` or `"r,g,b,a"` (`a` defaults to
    /// `1.0` when omitted), **on whichever scale the document used**.
    ///
    /// `colorType` accepts two numeric spellings — `0.0..=1.0` fractions
    /// and `0..=255` integers — as two `xsd:pattern`s over the same
    /// `xsd:string`, so nothing in the value itself says which was meant:
    /// `"1,0,0"` is a valid red on both scales, and `"1.0,0,0"` on the
    /// second. Reading them as one variant keeps that ambiguity where SUMO
    /// put it instead of resolving it with a guess that would be wrong for
    /// exactly the values where it matters. A consumer that needs
    /// fractions can divide by 255 when any component exceeds `1.0`.
    Rgba {
        r: f64,
        g: f64,
        b: f64,
        a: f64,
    },
    Named(NamedColor),
}

/// The symbolic color names `colorType` accepts as an alternative to RGBA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedColor {
    Red,
    Green,
    Blue,
    Yellow,
    Cyan,
    Magenta,
    Orange,
    White,
    Black,
    Grey,
    Invisible,
    Random,
}

/// A named path through the network: an ordered list of edges a [`Vehicle`]
/// can be assigned to by referencing this route's [`RouteId`].
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub id: RouteId,
    /// `None` when the route is defined by `<stop>` children alone instead
    /// (not modeled here — see the module docs).
    pub edges: Option<Vec<EdgeRef>>,
    pub color: Option<Color>,
}

/// A vehicle class definition (SUMO's `vType` element, `vTypeType` in the
/// XSD), shared by every [`Vehicle`] that references it via
/// [`Vehicle::vehicle_type`].
///
/// Only the handful of attributes most consumers care about are modeled.
/// `vTypeType` also has car-following-model parameters (`carFollowing-*`
/// child elements), lane-changing parameters (`lcStrategic`, ...),
/// emission/mass/shape details, and more — none of which are mapped here.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleType {
    pub id: VehicleTypeId,
    pub vehicle_class: Option<String>,
    pub length: Option<Length>,
    pub max_speed: Option<Velocity>,
    pub color: Option<Color>,
}

/// When a [`Vehicle`] enters the simulation: a fixed point in time, or one
/// of SUMO's symbolic triggers (`departType`'s non-numeric union member).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Depart {
    Time(Time),
    Triggered,
    ContainerTriggered,
    Split,
    Begin,
}

/// A single vehicle running a route (SUMO's `vehicle` element,
/// `vehicleType` in the XSD — not to be confused with [`VehicleType`],
/// *this* crate's type for SUMO's `vType`/`vTypeType`, a vehicle *class*
/// definition. The clash is SUMO's own naming, not introduced here).
///
/// Only the attributes covering the common case (a vehicle referencing a
/// pre-defined `vType` and `route` by id) are modeled. `vehicleType` also
/// allows the route to be given inline (a nested `<route>` or
/// `<routeDistribution>` element instead of the `route` attribute), and has
/// arrival/departure lane, position, and speed attributes, none of which
/// are mapped here.
#[derive(Debug, Clone, PartialEq)]
pub struct Vehicle {
    pub id: VehicleId,
    pub vehicle_type: Option<VehicleTypeId>,
    /// The route this vehicle runs, by id. `None` if the route was instead
    /// given inline on the `<vehicle>` element (not modeled — see above).
    pub route: Option<RouteId>,
    pub depart: Depart,
    pub color: Option<Color>,
}

/// A `.rou.xml` file's content, restricted to what this crate models — see
/// the module docs for what's dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Routes {
    pub vehicle_types: Vec<VehicleType>,
    pub routes: Vec<Route>,
    pub vehicles: Vec<Vehicle>,
}
