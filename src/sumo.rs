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

/// The inverse of [`parse_finite`]: formats a number the way SUMO's own
/// `floatType` pattern accepts.
///
/// No `NaN`/infinity guard on this side — every domain value reachable from
/// a parsed document already passed [`parse_finite`] on the way in, and a
/// hand-built domain value with a non-finite field is a caller bug this
/// crate has no way to see, the same way it doesn't re-validate any other
/// domain invariant at write time.
///
/// `f64`'s own [`Display`](std::fmt::Display) never falls back to
/// scientific notation the way `{:e}` would, so this is a thin, named
/// wrapper rather than a real conversion — its purpose is the same one
/// [`parse_finite`] serves on the read side: one place both directions of
/// "how SUMO spells a number" live, instead of a bare `.to_string()` at
/// every call site.
#[cfg(feature = "write")]
pub(crate) fn format_finite(value: f64) -> String {
    value.to_string()
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

    /// Formats this point back into SUMO's `positionType` encoding — the
    /// inverse of [`Self::parse`].
    ///
    /// Omits `z` when it is exactly `0.0`. [`Self::parse`] reads a
    /// 2-coordinate position and a 3-coordinate one with `z="0"` into the
    /// identical value, so there is no original spelling recorded to
    /// reproduce either way — and writing `z` on every point would make a
    /// flat, 2-D network (most of them) noisier than what `netconvert`
    /// itself emits for the same data.
    #[cfg(feature = "write")]
    pub(crate) fn format(&self) -> String {
        if self.z == 0.0 {
            format!("{},{}", format_finite(self.x), format_finite(self.y))
        } else {
            format!(
                "{},{},{}",
                format_finite(self.x),
                format_finite(self.y),
                format_finite(self.z)
            )
        }
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

/// The inverse of [`split_ids`]: joins a list of ids into SUMO's
/// whitespace-separated attribute value. Never `None` — for the *required*
/// list attributes [`split_ids`] reads (`incLanes`, `intLanes`, ...), an
/// empty list is a legal value (a dead-end junction's `incLanes=""`), not a
/// missing one, so the caller writes the (possibly empty) string as-is.
///
/// Generic over `T: AsRef<str>` rather than fixed to one id newtype, same
/// reasoning as [`split_ids`] being generic over `T: From<String>`.
#[cfg(feature = "write")]
pub(crate) fn join_ids<T: AsRef<str>>(ids: &[T]) -> String {
    ids.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(" ")
}

/// [`join_ids`] for an attribute SUMO may omit entirely: `None` for an empty
/// list, so the caller can leave the attribute out rather than writing
/// `attr=""` — the inverse of how [`split_ids_opt`] reads a missing
/// attribute back as an empty list. (A non-empty `raw` that splits to
/// nothing can't happen: [`split_ids`] only ever produces `Vec::new()` from
/// an all-whitespace string, and no writer using this helper would build one
/// of those from anything but an already-empty list.)
#[cfg(feature = "write")]
pub(crate) fn join_ids_opt<T: AsRef<str>>(ids: &[T]) -> Option<String> {
    (!ids.is_empty()).then(|| join_ids(ids))
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

/// The inverse of [`parse_bool`]: always the canonical `"true"`/`"false"`
/// spelling. [`parse_bool`] accepts SUMO's nine other spellings (`"True"`,
/// `"yes"`, `"1"`, ...) because a reader has to cope with whatever a
/// document contains, but nothing needs a *writer* to reproduce whichever
/// one happened to be on the way in — a `bool` doesn't remember its
/// spelling, so there is nothing to round-trip here the way [`Point::format`]
/// round-trips a position's coordinates.
#[cfg(feature = "write")]
pub(crate) fn format_bool(value: bool) -> crate::schema::BoolType {
    if value {
        crate::schema::BoolType::true_
    } else {
        crate::schema::BoolType::false_
    }
}

/// [`format_bool`] for an attribute this crate models as `Option<bool>`.
#[cfg(feature = "write")]
pub(crate) fn format_bool_opt(value: Option<bool>) -> Option<crate::schema::BoolType> {
    value.map(format_bool)
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

    #[cfg(feature = "write")]
    mod write_tests {
        use super::*;

        #[test]
        fn formats_a_point_omitting_z_only_when_it_is_zero() {
            assert_eq!(
                Point {
                    x: 1.5,
                    y: -2.0,
                    z: 0.0
                }
                .format(),
                "1.5,-2"
            );
            assert_eq!(
                Point {
                    x: 1.5,
                    y: -2.0,
                    z: 3.0
                }
                .format(),
                "1.5,-2,3"
            );
        }

        #[test]
        fn every_parsed_position_formats_back_to_an_equal_point() {
            for raw in ["0,0", "42,7,1.5", "-1.00,-1.00"] {
                let point = Point::parse(raw, "test").unwrap();
                assert_eq!(Point::parse(&point.format(), "test").unwrap(), point);
            }
        }

        #[test]
        fn joins_and_splits_are_inverse_on_a_nonempty_list() {
            let ids = ["a", "b", "c"];
            assert_eq!(join_ids(&ids), "a b c");
            assert_eq!(split_ids::<String>(&join_ids(&ids)), vec!["a", "b", "c"]);
        }

        #[test]
        fn join_ids_opt_omits_the_attribute_for_an_empty_list() {
            assert_eq!(join_ids_opt::<String>(&[]), None);
            assert_eq!(join_ids_opt(&["a".to_string()]), Some("a".to_string()));
        }

        #[test]
        fn formats_the_canonical_bool_spelling() {
            // `BoolType` isn't `PartialEq` (it's generated, see `lib.rs`'s
            // `mod schema` docs), so round-trip through `parse_bool` instead
            // of comparing variants directly.
            assert!(parse_bool(format_bool(true)).unwrap());
            assert!(!parse_bool(format_bool(false)).unwrap());
            assert!(format_bool_opt(None).is_none());
        }
    }
}
