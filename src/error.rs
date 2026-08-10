//! This crate's error type.
//!
//! A single typed enum rather than an erased one (`anyhow::Error`, which
//! earlier versions returned): the same argument the `schema` module's docs
//! in `lib.rs` make for keeping `xsd-parser-types` out of the public API
//! applies to an error crate too. `anyhow::Error` in a `pub fn`'s signature
//! makes `anyhow` part of this crate's API — a major bump there would be a
//! breaking change here — and it gives consumers nothing to match on, so
//! "the file was missing" and "a coordinate was `NaN`" are distinguishable
//! only by string-matching the message. `thiserror` generates the
//! [`std::error::Error`] plumbing for the enum below without appearing in
//! any signature at all: it is a `derive`-only dependency, so it can be
//! bumped freely.
//!
//! [`Error`] is [`non_exhaustive`](https://doc.rust-lang.org/reference/attributes/type_system.html)
//! so that modelling more of SUMO's formats later — every one of which
//! brings its own malformed-input cases — doesn't force a breaking release
//! each time.

use std::path::PathBuf;

/// Convenience alias for a [`Result`](std::result::Result) with this
/// crate's [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Anything that can go wrong reading or writing a SUMO file.
///
/// The variants split into three groups: I/O against the file itself,
/// well-formedness of the XML and its root element, and the SUMO-specific
/// value encodings each format's `schema_mapper` interprets (positions,
/// shapes, booleans, ...). Only the last group is specific to this crate;
/// the first two would apply to any XML reader.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The input file could not be opened.
    #[error("could not open input file: {path}")]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The output file could not be created.
    #[error("could not create output file: {path}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Writing to the output failed partway through.
    ///
    /// `path` is `None` when writing to a caller-supplied sink
    /// (`write_network_to` and friends) rather than to a file this crate
    /// opened itself. Code span, not an intra-doc link: this module is
    /// compiled under every feature combination, and `write_network_to`
    /// only exists under `net` + `write` — see the crate docs.
    #[error("failed to write {what}{}", OptionalPath(path))]
    Write {
        what: &'static str,
        path: Option<PathBuf>,
        #[source]
        source: std::io::Error,
    },

    /// The document isn't well-formed XML, or doesn't fit the format's
    /// schema.
    ///
    /// The underlying error is boxed rather than named: it comes from
    /// `xsd-parser-types`, which `lib.rs` deliberately keeps out of this
    /// crate's public API. [`std::error::Error::source`] still reaches it
    /// for anyone who wants the detail.
    #[error("failed to parse {what}{}", OptionalPath(path))]
    Parse {
        what: &'static str,
        path: Option<PathBuf>,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The value could not be serialized to XML. Boxed for the same reason
    /// as [`Self::Parse`].
    #[error("failed to serialize {what}")]
    Serialize {
        what: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The document is well-formed but rooted at the wrong element — a
    /// `.rou.xml` handed to `read_network`, say. (Code span rather than a
    /// link, for the same reason as [`Self::Write`] above.)
    ///
    /// Worth its own variant because nothing else catches it: xsd-parser
    /// generates deserializers for XSD *types*, not elements, and most SUMO
    /// schemas make their content optional, so an unrelated document
    /// otherwise parses into an empty, plausible-looking value. See
    /// `xml::RootRecordingReader`.
    #[error("not a {what}: expected a <{expected}> root element, found <{found}>")]
    WrongRoot {
        what: &'static str,
        expected: &'static str,
        found: String,
    },

    /// The document has no root element at all (it is empty, or contains
    /// only a declaration and comments).
    #[error("not a {what}: the document has no root element")]
    MissingRoot { what: &'static str },

    /// An attribute the format requires in practice was absent. Only raised
    /// where a real SUMO writer always emits the attribute even though the
    /// XSD marks it optional — see `net::domain::Location`.
    #[error("<{element}> is missing {attribute}")]
    MissingAttribute {
        element: &'static str,
        attribute: &'static str,
    },

    /// A number didn't parse. `context` names the attribute it came from
    /// (`"junction x coordinate"`, ...).
    #[error("invalid {context}: {value:?}")]
    InvalidNumber {
        context: &'static str,
        value: String,
    },

    /// A number parsed but was `NaN` or an infinity, which SUMO's
    /// `floatType` pattern excludes and which would poison every
    /// computation downstream — see `sumo::parse_finite`.
    #[error("non-finite {context}: {value:?}")]
    NonFiniteNumber {
        context: &'static str,
        value: String,
    },

    /// A position had fewer than the two coordinates SUMO's `positionType`
    /// requires.
    #[error("incomplete {context}: {value:?}")]
    IncompletePosition {
        context: &'static str,
        value: String,
    },

    /// A position had more than the three coordinates SUMO's `positionType`
    /// allows.
    #[error("{context} with too many coordinates: {value:?}")]
    TooManyCoordinates {
        context: &'static str,
        value: String,
    },

    /// SUMO's `boolType` allows `"x"` and `"-"`, the "unspecified" marker
    /// its configuration options use. Neither has a binary meaning, so they
    /// are an error rather than a silent `false`.
    #[error("SUMO boolean value with no binary meaning (\"x\"/\"-\")")]
    AmbiguousBool,

    /// A `locationType` boundary didn't have exactly the four components
    /// `"minX,minY,maxX,maxY"` needs.
    #[error("SUMO boundary must have 4 components: {value:?}")]
    InvalidBoundary { value: String },

    /// A `colorType` in its numeric form didn't have 3 (`"r,g,b"`) or 4
    /// (`"r,g,b,a"`) components.
    #[error("SUMO color must have 3 or 4 components: {value:?}")]
    InvalidColor { value: String },

    /// A `timeType` in its clock form didn't have 3 (`"H:M:S"`) or 4
    /// (`"H:M:S:MS"`) components.
    #[error("SUMO clock time must have 3 or 4 components: {value:?}")]
    InvalidClockTime { value: String },

    /// Two attributes SUMO treats as alternatives were both set — an
    /// `e2Detector` with both `lane` and `lanes`, for instance.
    #[error("{element} sets both `{first}` and `{second}`, which are alternatives")]
    ConflictingAttributes {
        element: &'static str,
        first: &'static str,
        second: &'static str,
    },
}

impl Error {
    /// Names the file this error came from, for the variants that have
    /// somewhere to put it ([`Self::Parse`] and [`Self::Write`]).
    ///
    /// Lets `xml::read_document_at`/`write_document_at` add the path on the
    /// way out without every layer below them having to thread an
    /// `Option<&Path>` through just in case. A no-op for the rest: the
    /// value-level variants ([`Self::NonFiniteNumber`], ...) deliberately
    /// don't carry a path — they already name the offending attribute *and*
    /// its value, and a caller who passed the path to `read_network` in the
    /// first place knows which file it was.
    pub(crate) fn with_path(mut self, path: &std::path::Path) -> Self {
        if let Self::Parse { path: slot, .. } | Self::Write { path: slot, .. } = &mut self {
            *slot = Some(path.to_path_buf());
        }

        self
    }
}

/// Renders `" in <path>"` for the variants that may or may not have a file
/// to name, and nothing when they don't.
///
/// A [`Display`](std::fmt::Display) adapter rather than two near-identical
/// variants per case (one with a path, one without): what went wrong is the
/// same either way, and whether this crate opened the file or the caller
/// handed over a sink isn't a different kind of failure.
struct OptionalPath<'a>(&'a Option<PathBuf>);

impl std::fmt::Display for OptionalPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(path) => write!(f, " in {}", path.display()),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_file_only_when_there_is_one() {
        let with_path = Error::Parse {
            what: "SUMO network",
            path: Some(PathBuf::from("city.net.xml")),
            source: "boom".into(),
        };
        assert_eq!(
            with_path.to_string(),
            "failed to parse SUMO network in city.net.xml"
        );

        let without_path = Error::Parse {
            what: "SUMO network",
            path: None,
            source: "boom".into(),
        };
        assert_eq!(without_path.to_string(), "failed to parse SUMO network");
    }

    #[test]
    fn keeps_the_underlying_error_reachable_as_a_source() {
        use std::error::Error as _;

        let error = Error::Parse {
            what: "SUMO network",
            path: None,
            source: "the real detail".into(),
        };
        assert_eq!(error.source().unwrap().to_string(), "the real detail");
    }
}
