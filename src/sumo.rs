//! Primitives shared by every format's `schema_mapper`.
//!
//! What lives here is knowledge about *SUMO*, not about any one of its file
//! formats: how SUMO spells a boolean, how it encodes a number, how it
//! writes a list of ids. Each format's mapper is otherwise its own code —
//! the bulk of a mapper is "take these attributes, build this struct", and
//! that part is genuinely different per format.
//!
//! The reason these need a home of their own rather than living in
//! whichever mapper got them first: the format features are independent, so
//! `additional` can't call into `net`. Before this module both of them
//! carried their own byte-identical copy of [`parse_finite`] and of the
//! boolean table, free to drift apart the first time one got a fix.

use crate::{Error, Result};

/// Parses one of SUMO's text-encoded numbers, rejecting `NaN` and the
/// infinities.
///
/// SUMO declares `floatType` in `types/base.xsd` as an `xsd:string`
/// restricted by a pattern rather than as an `xsd:float`, so xsd-parser
/// hands these over as text for the mappers to parse. The pattern excludes
/// `NaN` and `inf`, but xsd-parser enforces no `xsd:pattern`, and `f64`'s
/// `FromStr` happily accepts both — so without this guard a `NaN` would
/// flow into a position or a length and poison every computation
/// downstream, which is exactly what the typed layer exists to prevent.
///
/// `context` names the value in the error message (`"junction x
/// coordinate"`, ...).
pub(crate) fn parse_finite(raw: &str, context: &'static str) -> Result<f64> {
    let value: f64 = raw.trim().parse().map_err(|_| Error::InvalidNumber {
        context,
        value: raw.to_owned(),
    })?;

    if !value.is_finite() {
        return Err(Error::NonFiniteNumber {
            context,
            value: raw.to_owned(),
        });
    }

    Ok(value)
}

/// A point in the network's coordinate system (meters), with an optional
/// `z` represented here as `0.0` by default.
///
/// [`Default`] is the origin.
///
/// Shared by every format rather than defined once per format, unlike the
/// id newtypes next to it. Those are per-format on purpose — an
/// `additional`'s `LaneRef` and a `net`'s `LaneId` carry the same string but
/// mean "a lane in the file I came with" and "a lane I define", and keeping
/// them un-mixable is worth the duplication. A point has no such reading:
/// it is a pair of meters in one coordinate system, and `net`'s and
/// `additional`'s were byte-identical structs that only existed separately
/// because `additional` cannot name a type from `net`, which may not be
/// enabled. This module *is* always compiled when any format is (see
/// `lib.rs`), so it can hold the one definition both re-export — and with
/// it, in `Self::parse`, the single copy of how SUMO spells a position.
/// (Plain code span, not a link: `parse` is `pub(crate)`, so an intra-doc
/// link to it fails `rustdoc::private_intra_doc_links` under `-D warnings`.)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point {
    /// Parses SUMO's `positionType` encoding: `"x,y"` or `"x,y,z"`, with `z`
    /// reading as `0.0` when omitted.
    ///
    /// Takes the `context` its errors should name (`"SUMO position"`,
    /// `"detector icon position"`, ...), which is why this is an inherent
    /// constructor and [`TryFrom<&str>`] below is a thin wrapper over it
    /// rather than the other way round: the same encoding spells several
    /// different attributes, and "invalid position" without saying *which*
    /// is the kind of error that sends someone reading a 200 MB `.net.xml`
    /// by hand. Crate-internal for that reason — the context argument is an
    /// error-message detail, not something a consumer should have to pass.
    pub(crate) fn parse(raw: &str, context: &'static str) -> Result<Self> {
        let mut parts = raw.split(',');
        let mut next_coordinate = || -> Result<f64> {
            let part = parts.next().ok_or(Error::IncompletePosition {
                context,
                value: raw.to_owned(),
            })?;
            parse_finite(part, context)
        };

        let x = next_coordinate()?;
        let y = next_coordinate()?;
        let z = match parts.next() {
            Some(z) => parse_finite(z, context)?,
            None => 0.0,
        };

        if parts.next().is_some() {
            return Err(Error::TooManyCoordinates {
                context,
                value: raw.to_owned(),
            });
        }

        Ok(Point { x, y, z })
    }
}

/// `Point::parse` under the generic name, for the common case where "SUMO
/// position" is context enough. Lives here beside the type rather than in a
/// format's `schema_mapper` so there is one impl of it, not one per format
/// competing for the same `impl TryFrom<&str> for Point`.
impl TryFrom<&str> for Point {
    type Error = Error;

    fn try_from(raw: &str) -> Result<Self> {
        Point::parse(raw, "SUMO position")
    }
}

/// Splits one of SUMO's whitespace-separated id lists (`incLanes`,
/// `intLanes`, `edges`, `lanes`, ...) into the newtype the caller expects.
///
/// Generic over that newtype rather than fixed to one, because each format
/// has its own — `net`'s `LaneId`, `routes`' `EdgeRef`, `additional`'s
/// `LaneRef` — and they are deliberately unrelated types.
pub(crate) fn split_ids<T: From<String>>(raw: &str) -> Vec<T> {
    raw.split_whitespace()
        .map(|id| T::from(id.to_owned()))
        .collect()
}

/// [`split_ids`] for an attribute SUMO may omit entirely; absent reads as
/// the empty list.
pub(crate) fn split_ids_opt<T: From<String>>(raw: Option<&str>) -> Vec<T> {
    raw.map(split_ids).unwrap_or_default()
}

/// Converts SUMO's permissive `boolType` spelling into a real `bool`.
///
/// SUMO accepts ten spellings across five meanings, in both cases, plus two
/// values that aren't booleans at all: `"x"` and `"-"` are the
/// "unspecified" marker its configuration options use, and have no binary
/// equivalent, so they are an error rather than a silent `false`.
pub(crate) fn parse_bool(value: crate::schema::BoolType) -> Result<bool> {
    use crate::schema::BoolType as S;

    match value {
        S::true_ | S::True | S::yes | S::on | S::_1 => Ok(true),
        S::false_ | S::False | S::no | S::off | S::_0 => Ok(false),
        S::x | S::Dash => Err(Error::AmbiguousBool),
    }
}

/// [`parse_bool`] for an attribute SUMO may omit. Absent stays absent
/// rather than being resolved to a default here: what the default *is*
/// depends on the attribute, so that call belongs to each format's mapper.
pub(crate) fn parse_bool_opt(value: Option<crate::schema::BoolType>) -> Result<Option<bool>> {
    value.map(parse_bool).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::BoolType;

    #[test]
    fn rejects_values_that_parse_as_numbers_but_are_not_finite() {
        assert!(parse_finite("NaN", "test").is_err());
        assert!(parse_finite("inf", "test").is_err());
        assert!(parse_finite("-inf", "test").is_err());
        assert!(parse_finite("not-a-number", "test").is_err());
    }

    #[test]
    fn accepts_the_shapes_sumo_actually_writes() {
        assert_eq!(parse_finite("12.50", "test").unwrap(), 12.5);
        assert_eq!(parse_finite("-3", "test").unwrap(), -3.0);
        assert_eq!(parse_finite("  7.0  ", "test").unwrap(), 7.0);
    }

    #[test]
    fn parses_positions_with_and_without_z() {
        assert_eq!(
            Point::parse("42,7", "test").unwrap(),
            Point {
                x: 42.0,
                y: 7.0,
                z: 0.0
            }
        );
        assert_eq!(
            Point::parse("42,7,1.5", "test").unwrap(),
            Point {
                x: 42.0,
                y: 7.0,
                z: 1.5
            }
        );
        assert_eq!(
            Point::parse("-1.00,-1.00", "test").unwrap(),
            Point {
                x: -1.0,
                y: -1.0,
                z: 0.0
            }
        );
    }

    #[test]
    fn rejects_positions_of_the_wrong_shape_or_with_poisoned_coordinates() {
        assert!(Point::parse("42", "test").is_err());
        assert!(Point::parse("42,7,1,9", "test").is_err());
        assert!(Point::parse("NaN,7", "test").is_err());
        assert!(Point::parse("42,inf", "test").is_err());
    }

    #[test]
    fn splits_on_any_run_of_whitespace() {
        #[derive(Debug, PartialEq)]
        struct Id(String);
        impl From<String> for Id {
            fn from(s: String) -> Self {
                Id(s)
            }
        }

        let ids: Vec<Id> = split_ids("a  b\tc");
        assert_eq!(ids, vec![Id("a".into()), Id("b".into()), Id("c".into())]);
        assert!(split_ids::<Id>("").is_empty());
        assert!(split_ids_opt::<Id>(None).is_empty());
    }

    #[test]
    fn distinguishes_case_sensitive_bool_spellings() {
        assert!(parse_bool(BoolType::True).unwrap());
        assert!(parse_bool(BoolType::true_).unwrap());
        assert!(!parse_bool(BoolType::False).unwrap());
        assert!(!parse_bool(BoolType::false_).unwrap());
    }

    #[test]
    fn rejects_bool_without_binary_meaning() {
        assert!(parse_bool(BoolType::x).is_err());
        assert!(parse_bool(BoolType::Dash).is_err());
        assert_eq!(parse_bool_opt(None).unwrap(), None);
    }
}
