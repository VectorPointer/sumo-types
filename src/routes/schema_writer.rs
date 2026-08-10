//! Conversion from layer 2 ([`crate::routes::domain`]) to layer 1 (the
//! private `schema` module) — the inverse of `schema_mapper`, only compiled
//! under `write`.
//!
//! Every attribute this crate's `domain::Route`/`VehicleType`/`Vehicle`
//! doesn't model (`vTypeType`'s ~170 car-following/lane-changing
//! parameters, `route`'s `<stop>` children, ...) is written as its SUMO
//! default (an absent optional attribute, an empty child list) — the same
//! thing `read_routes` already treats an absent one as. See the `domain`
//! module docs for the exact list.
//!
//! Conversions borrow (`TryFrom<&domain::X>`) rather than consume: a
//! `write_routes` caller almost always wants to keep using its `Routes`
//! afterward (log it, write it a second time to a different path, ...), and
//! borrowing costs this module only the odd `.clone()` on a `String` schema
//! itself owns.

use super::domain::{Color, Depart, NamedColor, Route, Routes, Vehicle, VehicleType};
use crate::schema;
use crate::sumo::{format_finite, join_ids};
use crate::{Error, Result};
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// The reverse of `schema_mapper`'s `From<schema::ColorType6Type> for
/// NamedColor`. SUMO accepts both `"grey"` and `"gray"` on read; this always
/// writes `"grey"` — [`NamedColor::Grey`] doesn't remember which spelling a
/// document used, so there is nothing to round-trip, same reasoning as
/// [`crate::sumo::format_bool`] always writing `"true"`/`"false"`.
impl From<&NamedColor> for schema::ColorType6Type {
    fn from(value: &NamedColor) -> Self {
        use schema::ColorType6Type as S;
        match value {
            NamedColor::Red => S::red,
            NamedColor::Green => S::green,
            NamedColor::Blue => S::blue,
            NamedColor::Yellow => S::yellow,
            NamedColor::Cyan => S::cyan,
            NamedColor::Magenta => S::magenta,
            NamedColor::Orange => S::orange,
            NamedColor::White => S::white,
            NamedColor::Black => S::black,
            NamedColor::Grey => S::grey,
            NamedColor::Invisible => S::invisible,
            NamedColor::Random => S::random,
        }
    }
}

/// Formats [`Color::Rgba`]'s numeric form, on whichever scale the value
/// carries — see that variant's own docs for why this crate doesn't resolve
/// that ambiguity on read either.
impl From<&Color> for schema::ColorType {
    fn from(value: &Color) -> Self {
        match value {
            Color::Rgba { r, g, b, a } if *a == 1.0 => schema::ColorType::ColorType4(format!(
                "{},{},{}",
                format_finite(*r),
                format_finite(*g),
                format_finite(*b)
            )),
            Color::Rgba { r, g, b, a } => schema::ColorType::ColorType4(format!(
                "{},{},{},{}",
                format_finite(*r),
                format_finite(*g),
                format_finite(*b),
                format_finite(*a)
            )),
            Color::Named(named) => {
                schema::ColorType::ColorType6(schema::ColorType6Type::from(named))
            }
        }
    }
}

/// The reverse of `schema_mapper`'s `TryFrom<schema::DepartType> for
/// Depart`. [`Depart::Time`] always writes SUMO's plain-seconds spelling
/// (`sumoTimeType`'s `nonNegativeFloatType` member) rather than the
/// `"H:M:S"` alternative — `schema_mapper`'s `parse_clock_time` accepts the
/// latter on read because a document might use it, but nothing needs a
/// *writer* to reproduce it, and every SUMO tool this crate has seen emits
/// the numeric form.
impl From<&Depart> for schema::DepartType {
    fn from(value: &Depart) -> Self {
        use schema::DepartType13Type as Trigger;
        match value {
            Depart::Time(time) => schema::DepartType::sumoTimeType(
                schema::SumoTimeType::nonNegativeFloatType(time.get::<second>() as f32),
            ),
            Depart::Triggered => schema::DepartType::DepartType13(Trigger::triggered),
            Depart::ContainerTriggered => {
                schema::DepartType::DepartType13(Trigger::containerTriggered)
            }
            Depart::Split => schema::DepartType::DepartType13(Trigger::split),
            Depart::Begin => schema::DepartType::DepartType13(Trigger::begin),
        }
    }
}

impl From<&Route> for schema::RouteType {
    fn from(value: &Route) -> Self {
        schema::RouteType {
            edges: value.edges.as_ref().map(|edges| join_ids(edges)),
            color: value.color.as_ref().map(schema::ColorType::from),
            exit_times: None,
            cost: None,
            savings: None,
            repeat: None,
            cycle_time: None,
            probability: None,
            route_length: None,
            id: value.id.0.clone(),
            content: Vec::new(),
        }
    }
}

/// Every field `vTypeType` has beyond the handful [`VehicleType`] models
/// (car-following parameters, lane-changing parameters, emission/mass/shape
/// details, ...) defaults to absent. Kept as its own [`Default`] impl,
/// rather than spelled out at the one call site in [`VehicleType`]'s own
/// conversion below, because `vTypeType` alone has ~170 attributes — most of
/// this crate's structs are small enough that writing every field inline
/// is the more direct and greppable form (see `RouteType`'s conversion
/// above), but at this size a named `Default` earns its keep.
impl Default for schema::VTypeType {
    fn default() -> Self {
        schema::VTypeType {
            id: String::new(),
            length: None,
            min_gap: None,
            max_speed: None,
            desired_max_speed: None,
            probability: None,
            speed_factor: None,
            speed_dev: None,
            v_class: None,
            emission_class: None,
            maneuver_angle_times: None,
            gui_shape: None,
            width: None,
            height: None,
            mass: None,
            color: None,
            accel: None,
            decel: None,
            emergency_decel: None,
            apparent_decel: None,
            max_accel_profile: None,
            des_accel_profile: None,
            parking_badges: None,
            person_capacity: None,
            container_capacity: None,
            boarding_duration: None,
            loading_duration: None,
            scale: None,
            lc_strategic: None,
            lc_cooperative: None,
            lc_speed_gain: None,
            lc_keep_right: None,
            lc_sublane: None,
            lc_opposite: None,
            lc_pushy: None,
            lc_pushy_gap: None,
            lc_strategic_lookahead: None,
            lc_assertive: None,
            lc_lookahead_left: None,
            lc_speed_gain_right: None,
            lc_speed_gain_lookahead: None,
            lc_speed_gain_remain_time: None,
            lc_speed_gain_urgency: None,
            lc_cooperative_roundabout: None,
            lc_cooperative_speed: None,
            lc_turn_alignment_distance: None,
            lc_impatience: None,
            lc_time_to_impatience: None,
            lc_accel_lat: None,
            lc_max_speed_lat_standing: None,
            lc_max_speed_lat_factor: None,
            lc_max_dist_lat_standing: None,
            lc_overtake_right: None,
            lc_lane_discipline: None,
            lc_sigma: None,
            lc_keep_right_acceptance_time: None,
            lc_overtake_delta_speed_factor: None,
            lc_cont_right: None,
            max_speed_lat: None,
            lat_alignment: None,
            action_step_length: None,
            has_driver_state: None,
            min_gap_lat: None,
            jm_crossing_gap: None,
            jm_drive_after_yellow_time: None,
            jm_drive_after_red_time: None,
            jm_drive_red_speed: None,
            jm_ignore_keep_clear_time: None,
            jm_ignore_foe_speed: None,
            jm_ignore_foe_prob: None,
            jm_ignore_junction_foe_prob: None,
            jm_sigma_minor: None,
            jm_stopline_gap: None,
            jm_stopline_gap_minor: None,
            jm_timegap_minor: None,
            jm_extra_gap: None,
            jm_advance: None,
            jm_stop_sign_wait: None,
            jm_allway_stop_wait: None,
            sigma: None,
            sigma_step: None,
            impatience: None,
            tau: None,
            delta: None,
            stepping: None,
            adapt_time: None,
            adapt_factor: None,
            tmp_1: None,
            tmp_2: None,
            tmp_3: None,
            tmp_4: None,
            tmp_5: None,
            tau_last: None,
            ap_prob: None,
            k: None,
            phi: None,
            security: None,
            estimation: None,
            speed_control_gain: None,
            speed_control_min_gap: None,
            gap_closing_control_gain_speed: None,
            gap_closing_control_gain_space: None,
            gap_control_gain_speed: None,
            gap_control_gain_space: None,
            collision_avoidance_gain_speed: None,
            collision_avoidance_gain_space: None,
            collision_avoidance_override: None,
            tau_cacc_to_acc: None,
            apply_driver_state: None,
            car_follow_model: None,
            train_type: None,
            lane_change_model: None,
            img_file: None,
            osg_file: None,
            cc_1: None,
            cc_2: None,
            cc_3: None,
            cc_4: None,
            cc_5: None,
            cc_6: None,
            cc_7: None,
            cc_8: None,
            cc_9: None,
            c1: None,
            cc_decel: None,
            const_spacing: None,
            kp: None,
            lambda: None,
            omega_n: None,
            tau_engine: None,
            xi: None,
            lanes_count: None,
            cc_accel: None,
            ploeg_kp: None,
            ploeg_kd: None,
            ploeg_h: None,
            flatbed_ka: None,
            flatbed_kv: None,
            flatbed_kp: None,
            flatbed_d: None,
            flatbed_h: None,
            collision_min_gap_factor: None,
            speed_control_gain_cacc: None,
            gap_closing_control_gain_gap: None,
            gap_closing_control_gain_gap_dot: None,
            gap_control_gain_gap: None,
            gap_control_gain_gap_dot: None,
            collision_avoidance_gain_gap: None,
            collision_avoidance_gain_gap_dot: None,
            t_pers_drive: None,
            tpreview: None,
            treaction: None,
            t_pers_estimate: None,
            ccoolness: None,
            sigmaleader: None,
            sigmagap: None,
            sigmaerror: None,
            jerkmax: None,
            epsilonacc: None,
            taccmax: None,
            mflatness: None,
            mbegin: None,
            vehdynamics: None,
            maxvehpreview: None,
            startup_delay: None,
            time_to_teleport: None,
            time_to_teleport_bidi: None,
            speed_factor_premature: None,
            boarding_factor: None,
            speed_table: None,
            traction_table: None,
            resistance_table: None,
            mass_factor: None,
            max_power: None,
            max_traction: None,
            res_coef_constant: None,
            res_coef_linear: None,
            res_coef_quadratic: None,
            content: schema::VTypeTypeContent {
                param: Vec::new(),
                v_type_car_following_group: None,
            },
        }
    }
}

impl From<&VehicleType> for schema::VTypeType {
    fn from(value: &VehicleType) -> Self {
        schema::VTypeType {
            id: value.id.0.clone(),
            length: value.length.map(|l| l.get::<meter>() as f32),
            max_speed: value.max_speed.map(|s| s.get::<meter_per_second>() as f32),
            v_class: value.vehicle_class.clone(),
            color: value.color.as_ref().map(schema::ColorType::from),
            ..schema::VTypeType::default()
        }
    }
}

impl From<&Vehicle> for schema::VehicleType {
    fn from(value: &Vehicle) -> Self {
        schema::VehicleType {
            id: value.id.0.clone(),
            route: None,
            reroute: None,
            from_taz: None,
            to_taz: None,
            via: None,
            type_: value.vehicle_type.as_ref().map(|id| id.0.clone()),
            depart: schema::DepartType::from(&value.depart),
            color: value.color.as_ref().map(schema::ColorType::from),
            depart_lane: None,
            depart_pos: None,
            depart_speed: None,
            depart_edge: None,
            arrival_edge: None,
            arrival_lane: None,
            arrival_pos: None,
            arrival_speed: None,
            depart_pos_lat: None,
            arrival_pos_lat: None,
            arrival: None,
            route_length: None,
            line: None,
            person_number: None,
            container_number: None,
            speed_factor: None,
            insertion_checks: None,
            parking_badges: None,
            content: schema::VehicleTypeContent {
                param: Vec::new(),
                vehicle_route_choice_group: None,
                vehicle_stop_group: None,
            },
        }
        // `route` above is deliberately left `None`, not
        // `value.route.map(|id| id.0.clone())`: `schema::VehicleType.route`
        // and `.content` (an inline `<route>`/`<routeDistribution>`) are
        // SUMO's alternative spellings of the same thing, so setting
        // `route` is done by the caller assembling `RoutesTypeContent`
        // below, once, rather than duplicated into every `From` impl that
        // touches a vehicle.
    }
}

impl TryFrom<&Routes> for schema::RoutesType {
    type Error = Error;

    fn try_from(value: &Routes) -> Result<Self> {
        let vehicle_types = value
            .vehicle_types
            .iter()
            .map(schema::VTypeType::from)
            .map(Some)
            .map(schema::RoutesTypeContent::vType);

        let routes = value
            .routes
            .iter()
            .map(schema::RouteType::from)
            .map(Some)
            .map(schema::RoutesTypeContent::route);

        let vehicles = value.vehicles.iter().map(|vehicle| {
            let mut raw = schema::VehicleType::from(vehicle);
            raw.route = vehicle.route.as_ref().map(|id| id.0.clone());
            schema::RoutesTypeContent::vehicle(Some(raw))
        });

        Ok(schema::RoutesType {
            content: vehicle_types.chain(routes).chain(vehicles).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::domain::{EdgeRef, RouteId, VehicleId, VehicleTypeId};

    #[test]
    fn formats_named_color_using_the_canonical_spelling() {
        // `schema::ColorType6Type` isn't `PartialEq` (it's generated — see
        // `lib.rs`'s `mod schema` docs), so round-trip through `NamedColor`
        // instead of comparing variants directly.
        use crate::routes::domain::NamedColor;
        let raw = schema::ColorType6Type::from(&NamedColor::Grey);
        assert_eq!(NamedColor::from(raw), NamedColor::Grey);
    }

    #[test]
    fn formats_rgba_omitting_alpha_only_when_opaque() {
        let opaque = schema::ColorType::from(&Color::Rgba {
            r: 0.8,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        });
        assert!(matches!(opaque, schema::ColorType::ColorType4(ref s) if s == "0.8,0.1,0.1"));

        let translucent = schema::ColorType::from(&Color::Rgba {
            r: 0.8,
            g: 0.1,
            b: 0.1,
            a: 0.5,
        });
        assert!(
            matches!(translucent, schema::ColorType::ColorType4(ref s) if s == "0.8,0.1,0.1,0.5")
        );
    }

    #[test]
    fn writes_depart_as_plain_seconds() {
        use uom::si::f64::Time;

        let raw = schema::DepartType::from(&Depart::Time(Time::new::<second>(12.5)));
        assert!(matches!(
            raw,
            schema::DepartType::sumoTimeType(schema::SumoTimeType::nonNegativeFloatType(s))
                if s == 12.5
        ));
    }

    #[test]
    fn writes_a_route_with_no_edges_as_a_missing_attribute() {
        let route = Route {
            id: RouteId("r0".into()),
            edges: None,
            color: None,
        };
        assert_eq!(schema::RouteType::from(&route).edges, None);
    }

    #[test]
    fn writes_a_route_with_explicitly_empty_edges_as_an_empty_attribute() {
        // `Some(vec![])` (`edges=""` in the source document) and `None` (no
        // `edges` attribute at all — the route was defined via `<stop>`
        // children instead) are different `Route` values; the writer must
        // not collapse them back into the same output.
        let route = Route {
            id: RouteId("r0".into()),
            edges: Some(Vec::new()),
            color: None,
        };
        assert_eq!(schema::RouteType::from(&route).edges, Some(String::new()));
    }

    #[test]
    fn joins_a_routes_edge_list() {
        let route = Route {
            id: RouteId("r0".into()),
            edges: Some(vec![
                EdgeRef("e0".into()),
                EdgeRef("e1".into()),
                EdgeRef("e2".into()),
            ]),
            color: None,
        };
        assert_eq!(
            schema::RouteType::from(&route).edges,
            Some("e0 e1 e2".to_string())
        );
    }

    #[test]
    fn wires_a_vehicles_route_reference_through_routestypecontent() {
        let routes = Routes {
            vehicle_types: vec![],
            routes: vec![],
            vehicles: vec![Vehicle {
                id: VehicleId("v0".into()),
                vehicle_type: Some(VehicleTypeId("car".into())),
                route: Some(RouteId("r0".into())),
                depart: Depart::Begin,
                color: None,
            }],
        };

        let raw = schema::RoutesType::try_from(&routes).unwrap();
        let vehicle = raw
            .content
            .into_iter()
            .find_map(|item| match item {
                schema::RoutesTypeContent::vehicle(Some(v)) => Some(v),
                _ => None,
            })
            .unwrap();
        assert_eq!(vehicle.route.as_deref(), Some("r0"));
        assert_eq!(vehicle.type_.as_deref(), Some("car"));
    }
}
