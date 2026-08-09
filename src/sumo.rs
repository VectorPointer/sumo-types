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

use anyhow::{Context, Result, bail};

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
pub(crate) fn parse_finite(raw: &str, context: &str) -> Result<f64> {
    let value: f64 = raw
        .trim()
        .parse()
        .with_context(|| format!("invalid {context}: {raw:?}"))?;

    if !value.is_finite() {
        bail!("non-finite {context}: {raw:?}");
    }

    Ok(value)
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
        S::x | S::Dash => bail!("SUMO boolean value with no binary meaning (\"x\"/\"-\")"),
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
