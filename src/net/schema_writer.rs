//! Conversion from layer 2 ([`crate::domain`]) to layer 1 (the private
//! `schema` module) — the inverse of `schema_mapper`, only compiled under
//! `write`.
//!
//! `netconvert`'s own tool-configuration attributes on `<net>` itself
//! (`junctionCornerDetail`, `limitTurnSpeed`, ...) aren't modelled by
//! [`Network`] and are always written absent, same as every other attribute
//! this crate doesn't read — see the `domain` module docs for the full
//! picture of what's modelled.
//!
//! Conversions borrow (`TryFrom<&domain::X>`) rather than consume, same
//! rationale as `routes`' and `additional`'s own `schema_writer`.

use super::domain::{
    Boundary, Connection, ConnectionDirection, Edge, EdgeFunction, JunctionKind, Lane, Location,
    Network, Phase, Projection, Roundabout, Shape, SpreadType, TrafficLightKind,
    TrafficLightOffset, TrafficLightProgram,
};
use crate::schema;
use crate::sumo::{Point, format_finite, join_ids};
use crate::{Error, Result};
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Formats [`Shape`] back into SUMO's `shapeType`/`shapeTypeTwo` encoding:
/// whitespace-separated positions (see [`Point::format`]).
fn format_shape(shape: &Shape) -> String {
    shape
        .0
        .iter()
        .map(Point::format)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Formats [`Boundary`] back into SUMO's `"minX,minY,maxX,maxY"` encoding.
fn format_boundary(boundary: &Boundary) -> String {
    format!(
        "{},{},{},{}",
        format_finite(boundary.min.0),
        format_finite(boundary.min.1),
        format_finite(boundary.max.0),
        format_finite(boundary.max.1)
    )
}

/// The inverse of `schema_mapper`'s `From<String> for Projection`: `"!"` for
/// [`Projection::None`], the PROJ string as-is otherwise.
impl From<&Projection> for String {
    fn from(value: &Projection) -> Self {
        match value {
            Projection::None => "!".to_string(),
            Projection::Proj4(proj) => proj.clone(),
        }
    }
}

impl From<&Location> for schema::LocationType {
    fn from(value: &Location) -> Self {
        schema::LocationType {
            net_offset: Some(Point::format(&value.net_offset)),
            conv_boundary: Some(format_boundary(&value.converted_boundary)),
            orig_boundary: Some(format_boundary(&value.original_boundary)),
            proj_parameter: String::from(&value.projection),
        }
    }
}

impl From<&Lane> for schema::LaneType {
    fn from(value: &Lane) -> Self {
        schema::LaneType {
            id: value.id.0.clone(),
            index: value.index.0,
            allow: join_ids_opt_owned(&value.allow),
            disallow: join_ids_opt_owned(&value.disallow),
            prefer: None,
            speed: value.speed.get::<meter_per_second>() as f32,
            friction: None,
            length: value.length.get::<meter>() as f32,
            // Always written, not omitted when equal to
            // `default_lane_end_offset`/`default_lane_width`: unlike
            // `EdgeFunction::Normal` below (which really is indistinguishable
            // from "absent" on read), a lane whose width happens to equal
            // SUMO's default is a value this crate has no way to tell apart
            // from one that was never set — and writing it explicitly is the
            // direction that can't silently lose an intentional value.
            end_offset: Some(value.end_offset.get::<meter>() as f32),
            width: Some(value.width.get::<meter>() as f32),
            acceleration: None,
            shape: format_shape(&value.shape),
            custom_shape: None,
            type_: None,
            change_right: None,
            change_left: None,
            outline_shape: None,
            content: Vec::new(),
        }
    }
}

/// [`crate::sumo::join_ids_opt`] specialized to `Vec<String>` (`Lane::allow`/
/// `disallow` aren't one of this crate's id newtypes, just raw SUMO class
/// names) — `String` already implements `AsRef<str>`, so this is the same
/// call, named for the type it's used at.
fn join_ids_opt_owned(ids: &[String]) -> Option<String> {
    crate::sumo::join_ids_opt(ids)
}

impl From<&EdgeFunction> for Option<schema::EdgeTypeFunctionType> {
    /// [`EdgeFunction::Normal`] round-trips as an absent attribute, not an
    /// explicit `function="normal"`: `schema_mapper` already can't tell the
    /// two apart on read (`value.function.map_or(EdgeFunction::Normal, ...)`),
    /// so there is no distinction left here to preserve either way.
    fn from(value: &EdgeFunction) -> Self {
        use schema::EdgeTypeFunctionType as S;
        match value {
            EdgeFunction::Normal => None,
            EdgeFunction::Internal => Some(S::internal),
            EdgeFunction::Connector => Some(S::connector),
            EdgeFunction::Crossing => Some(S::crossing),
            EdgeFunction::Walkingarea => Some(S::walkingarea),
        }
    }
}

impl From<&SpreadType> for schema::SpreadTypeType {
    fn from(value: &SpreadType) -> Self {
        use schema::SpreadTypeType as S;
        match value {
            SpreadType::Center => S::center,
            SpreadType::RoadCenter => S::roadCenter,
            SpreadType::Right => S::right,
        }
    }
}

impl From<&Edge> for schema::EdgeType {
    fn from(value: &Edge) -> Self {
        let lanes = value.lanes.iter().map(schema::LaneType::from).collect();

        schema::EdgeType {
            id: value.id.0.clone(),
            function: Option::from(&value.function),
            from: value.from.as_ref().map(|id| id.0.clone()),
            to: value.to.as_ref().map(|id| id.0.clone()),
            name: value.name.clone(),
            priority: value.priority.map(|p| p.0),
            length: value.length.map(|l| l.get::<meter>() as f32),
            bidi: None,
            type_: None,
            routing_type: None,
            shape: value.shape.as_ref().map(format_shape),
            distance: None,
            spread_type: value.spread_type.as_ref().map(schema::SpreadTypeType::from),
            crossing_edges: None,
            content: vec![schema::EdgeTypeContent::lane(lanes)],
        }
    }
}

impl From<&JunctionKind> for schema::JunctionTypeType {
    fn from(value: &JunctionKind) -> Self {
        use schema::JunctionTypeType as S;
        match value {
            JunctionKind::TrafficLight => S::traffic_light,
            JunctionKind::TrafficLightUnregulated => S::traffic_light_unregulated,
            JunctionKind::TrafficLightRightOnRed => S::traffic_light_right_on_red,
            JunctionKind::RailSignal => S::rail_signal,
            JunctionKind::RailCrossing => S::rail_crossing,
            JunctionKind::Priority => S::priority,
            JunctionKind::PriorityStop => S::priority_stop,
            JunctionKind::LeftBeforeRight => S::left_before_right,
            JunctionKind::RightBeforeLeft => S::right_before_left,
            JunctionKind::AllWayStop => S::allway_stop,
            JunctionKind::Zipper => S::zipper,
            JunctionKind::District => S::district,
            JunctionKind::Unregulated => S::unregulated,
            JunctionKind::Internal => S::internal,
            JunctionKind::DeadEnd => S::dead_end,
        }
    }
}

impl From<&super::domain::Junction> for schema::JunctionType {
    fn from(value: &super::domain::Junction) -> Self {
        schema::JunctionType {
            id: value.id.0.clone(),
            x: format_finite(value.position.x),
            y: format_finite(value.position.y),
            z: (value.position.z != 0.0).then(|| format_finite(value.position.z)),
            type_: schema::JunctionTypeType::from(&value.kind),
            inc_lanes: join_ids(&value.incoming_lanes),
            int_lanes: join_ids(&value.internal_lanes),
            shape: value.shape.as_ref().map(format_shape),
            name: value.name.clone(),
            radius: None,
            custom_shape: None,
            right_of_way: None,
            fringe: None,
            roundabout: None,
            content: schema::JunctionTypeContent {
                request: Vec::new(),
                param: Vec::new(),
            },
        }
    }
}

impl From<&ConnectionDirection> for schema::ConnectionTypeDirType {
    fn from(value: &ConnectionDirection) -> Self {
        use schema::ConnectionTypeDirType as S;
        match value {
            ConnectionDirection::Straight => S::s,
            ConnectionDirection::Turn => S::t,
            ConnectionDirection::TurnLeftHand => S::T,
            ConnectionDirection::Left => S::l,
            ConnectionDirection::Right => S::r,
            ConnectionDirection::PartialLeft => S::L,
            ConnectionDirection::PartialRight => S::R,
        }
    }
}

impl From<&super::domain::LinkState> for schema::ConnectionTypeStateType {
    fn from(value: &super::domain::LinkState) -> Self {
        use super::domain::LinkState as D;
        use schema::ConnectionTypeStateType as S;
        match value {
            D::Major => S::M,
            D::Minor => S::m,
            D::TlsOffNoSignal => S::O,
            D::TlsOffBlinking => S::o,
            D::Equal => S::Eq,
            D::Stop => S::s,
            D::AllWayStop => S::w,
            D::Zipper => S::Z,
            D::DeadEnd => S::Dash,
        }
    }
}

impl From<&Connection> for schema::ConnectionType {
    fn from(value: &Connection) -> Self {
        schema::ConnectionType {
            from: value.from_edge.0.clone(),
            to: value.to_edge.0.clone(),
            from_lane: value.from_lane.0,
            to_lane: value.to_lane.0,
            // `pass`/`keep_clear` round-trip the same way `EdgeFunction::Normal`
            // does: `schema_mapper` resolves an absent attribute to SUMO's own
            // default (`false`/`true` respectively) and can't tell that apart
            // from the attribute being explicitly set to that same default, so
            // there is nothing lost by omitting it here too.
            pass: (value.pass).then_some(schema::BoolType::true_),
            keep_clear: (!value.keep_clear).then_some(schema::BoolType::false_),
            cont_pos: None,
            visibility: None,
            allow: None,
            disallow: None,
            speed: None,
            length: None,
            shape: None,
            uncontrolled: None,
            via: value.via.as_ref().map(|id| id.0.clone()),
            tl: value.traffic_light.as_ref().map(|id| id.0.clone()),
            link_index: value.link_index.map(|i| i.0),
            link_index_2: None,
            change_right: None,
            change_left: None,
            indirect: None,
            type_: None,
            dir: schema::ConnectionTypeDirType::from(&value.direction),
            state: schema::ConnectionTypeStateType::from(&value.state),
            content: Vec::new(),
        }
    }
}

impl From<&Roundabout> for schema::RoundaboutType {
    fn from(value: &Roundabout) -> Self {
        schema::RoundaboutType {
            nodes: join_ids(&value.nodes),
            edges: join_ids(&value.edges),
        }
    }
}

impl From<&TrafficLightKind> for schema::TlTypeType {
    fn from(value: &TrafficLightKind) -> Self {
        use schema::TlTypeType as S;
        match value {
            TrafficLightKind::Actuated => S::actuated,
            TrafficLightKind::DelayBased => S::delay_based,
            TrafficLightKind::Static => S::static_,
            TrafficLightKind::Nema => S::NEMA,
        }
    }
}

impl From<&TrafficLightOffset> for schema::OffsetType {
    fn from(value: &TrafficLightOffset) -> Self {
        match value {
            TrafficLightOffset::Fixed(time) => {
                schema::OffsetType::sumoFloatType(format_finite(time.get::<second>()))
            }
            TrafficLightOffset::Begin => {
                schema::OffsetType::OffsetType11(schema::OffsetType11Type::begin)
            }
        }
    }
}

impl From<&Phase> for schema::PhaseType {
    fn from(value: &Phase) -> Self {
        schema::PhaseType {
            duration: value.duration.get::<second>() as f32,
            min_dur: None,
            max_dur: None,
            earliest_end: None,
            latest_end: None,
            early_target: None,
            final_target: None,
            yellow: None,
            red: None,
            vehext: None,
            state: value.state.clone(),
            next: None,
            name: None,
        }
    }
}

impl From<&TrafficLightProgram> for schema::TlLogicType {
    fn from(value: &TrafficLightProgram) -> Self {
        let phases = value
            .phases
            .iter()
            .map(schema::PhaseType::from)
            .map(schema::TlLogicTypeContent::phase)
            .collect();

        schema::TlLogicType {
            id: value.id.0.clone(),
            type_: value.kind.as_ref().map(schema::TlTypeType::from),
            program_id: value.program_id.clone(),
            offset: value.offset.as_ref().map(schema::OffsetType::from),
            content: phases,
        }
    }
}

impl TryFrom<&Network> for schema::NetType {
    type Error = Error;

    fn try_from(value: &Network) -> Result<Self> {
        let edges = value.edges.iter().map(schema::EdgeType::from).collect();
        let junctions = value
            .junctions
            .iter()
            .map(schema::JunctionType::from)
            .collect();
        let connections = value
            .connections
            .iter()
            .map(schema::ConnectionType::from)
            .collect();
        let roundabouts = value
            .roundabouts
            .iter()
            .map(schema::RoundaboutType::from)
            .collect();
        let tl_logic = value
            .traffic_light_programs
            .iter()
            .map(schema::TlLogicType::from)
            .collect();

        Ok(schema::NetType {
            version: None,
            junction_corner_detail: None,
            junction_link_detail: None,
            lefthand: None,
            rectangular_lane_cut: None,
            walkingareas: None,
            limit_turn_speed: None,
            check_lane_foes_all: None,
            check_lane_foes_roundabout: None,
            tls_ignore_internal_junction_jam: None,
            spread_type: None,
            avoid_overlap: None,
            junction_higher_speed: None,
            internal_junctions_vehicle_width: None,
            junctions_minimal_shape: None,
            junctions_endpoint_shape: None,
            content: schema::NetTypeContent {
                location: schema::LocationType::from(&value.location),
                type_: Vec::new(),
                edge: edges,
                tl_logic,
                junction: junctions,
                connection: connections,
                prohibition: Vec::new(),
                roundabout: roundabouts,
                taz: Vec::new(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{JunctionId, JunctionKind, Point as DomainPoint};

    #[test]
    fn formats_a_shape_as_space_separated_positions() {
        let shape = Shape(vec![
            DomainPoint {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            DomainPoint {
                x: 100.0,
                y: 0.0,
                z: 0.0,
            },
        ]);
        assert_eq!(format_shape(&shape), "0,0 100,0");
    }

    #[test]
    fn formats_a_boundary_as_four_comma_separated_numbers() {
        let boundary = Boundary {
            min: (0.0, 0.0),
            max: (100.0, 50.5),
        };
        assert_eq!(format_boundary(&boundary), "0,0,100,50.5");
    }

    #[test]
    fn writes_edge_function_normal_as_an_absent_attribute() {
        // `schema::EdgeTypeFunctionType` isn't `PartialEq` (it's generated
        // — see `lib.rs`'s `mod schema` docs), so check with `matches!`
        // instead of comparing values directly.
        assert!(Option::<schema::EdgeTypeFunctionType>::from(&EdgeFunction::Normal).is_none());
        assert!(matches!(
            Option::<schema::EdgeTypeFunctionType>::from(&EdgeFunction::Internal),
            Some(schema::EdgeTypeFunctionType::internal)
        ));
    }

    #[test]
    fn writes_junction_z_only_when_nonzero() {
        let junction = super::super::domain::Junction {
            id: JunctionId("j0".into()),
            position: DomainPoint {
                x: 1.0,
                y: 2.0,
                z: 0.0,
            },
            kind: JunctionKind::DeadEnd,
            incoming_lanes: vec![],
            internal_lanes: vec![],
            shape: None,
            name: None,
        };
        assert_eq!(schema::JunctionType::from(&junction).z, None);

        let mut elevated = junction;
        elevated.position.z = 3.5;
        assert_eq!(
            schema::JunctionType::from(&elevated).z,
            Some("3.5".to_string())
        );
    }

    #[test]
    fn writes_connection_pass_and_keep_clear_only_when_they_differ_from_the_sumo_default() {
        let connection = Connection {
            from_edge: "e0".to_string().into(),
            to_edge: "e1".to_string().into(),
            from_lane: super::super::domain::LaneIndex(0),
            to_lane: super::super::domain::LaneIndex(0),
            direction: ConnectionDirection::Straight,
            state: super::super::domain::LinkState::Major,
            via: None,
            traffic_light: None,
            link_index: None,
            pass: false,
            keep_clear: true,
        };
        let raw = schema::ConnectionType::from(&connection);
        assert!(raw.pass.is_none());
        assert!(raw.keep_clear.is_none());

        let mut nondefault = connection;
        nondefault.pass = true;
        nondefault.keep_clear = false;
        let raw = schema::ConnectionType::from(&nondefault);
        assert!(matches!(raw.pass, Some(schema::BoolType::true_)));
        assert!(matches!(raw.keep_clear, Some(schema::BoolType::false_)));
    }
}
