//! Shared XML plumbing for every format's reader, and (under `write`) writer.
//!
//! xsd-parser generates deserializers for XSD *types*, not for elements, so
//! `schema::NetType::deserialize` will happily consume a document whose root
//! is named anything at all as long as its children fit the type. That makes
//! it possible to hand a `.rou.xml` (or any unrelated XML) to
//! [`crate::read_network`] and get back a plausible-looking, empty
//! `Network`. [`RootRecordingReader`] closes that gap.
//!
//! The write side has no equivalent problem — the caller names the root
//! element it wants (`"net"`, `"routes"`, `"additional"`) up front, rather
//! than a reader having to discover and validate it after the fact — so
//! [`write_document`] is a straight pipeline: layer 2 (domain) to layer 1
//! (schema) via each format's `schema_writer`, then to bytes via the
//! generated `WithSerializer`.

use crate::{Error, Result};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
#[cfg(feature = "write")]
use xsd_parser_types::quick_xml::{BytesDecl, SerializeSync, Writer};
use xsd_parser_types::quick_xml::{
    DeserializeSync, Error as XmlError, Event, IoReader, XmlReader, XmlReaderSync,
};

/// Wraps an [`XmlReaderSync`] and remembers the name of the first element it
/// sees, so the caller can check the document really is the format it asked
/// for.
///
/// Recording rather than rejecting on the spot keeps this a pass-through
/// reader: it never has to construct an `xsd-parser-types` error of its own,
/// and the deserializer sees exactly the event stream it would have seen
/// otherwise. The check happens in [`Self::root_name`], after deserializing.
pub(crate) struct RootRecordingReader<R> {
    inner: R,
    root: Option<Vec<u8>>,
}

impl<R> RootRecordingReader<R> {
    pub(crate) fn new(inner: R) -> Self {
        Self { inner, root: None }
    }

    /// Local name (namespace prefix stripped) of the document's root
    /// element, or `None` if no element was ever read — an empty or
    /// non-element document.
    pub(crate) fn root_name(&self) -> Option<&[u8]> {
        self.root.as_deref()
    }
}

impl<R: XmlReader> XmlReader for RootRecordingReader<R> {
    fn extend_error(&self, error: XmlError) -> XmlError {
        self.inner.extend_error(error)
    }
}

impl<'a, R: XmlReaderSync<'a>> XmlReaderSync<'a> for RootRecordingReader<R> {
    fn read_event(&mut self) -> Result<Event<'a>, XmlError> {
        let event = self.inner.read_event()?;

        if self.root.is_none() {
            // `Empty` as well as `Start`: a self-closing `<net/>` is still a
            // root element, and the XML declaration, comments and doctype
            // that precede it are neither.
            if let Event::Start(start) | Event::Empty(start) = &event {
                self.root = Some(start.local_name().as_ref().to_vec());
            }
        }

        Ok(event)
    }
}

/// Fails unless the document's root element is named `expected`.
///
/// `what` names the format in the error message (`"SUMO network"`, ...).
fn ensure_root_is<R>(
    reader: &RootRecordingReader<R>,
    expected: &'static str,
    what: &'static str,
) -> Result<()> {
    match reader.root_name() {
        Some(name) if name == expected.as_bytes() => Ok(()),
        Some(name) => Err(Error::WrongRoot {
            what,
            expected,
            found: String::from_utf8_lossy(name).into_owned(),
        }),
        None => Err(Error::MissingRoot { what }),
    }
}

/// The whole read pipeline every format's `read_*_from` runs: deserialize
/// `source` into the layer 1 type `S`, check the document really was rooted
/// at `<root>`, then convert to the layer 2 type `T`.
///
/// `S` is only ever a generated `schema` type and `T` its `domain`
/// counterpart, so the two parameters are always fixed at the call site;
/// the bounds spell out what that pairing has to provide.
///
/// The root check runs *after* deserializing, not before: the root name is
/// only known once the reader has produced its first element event.
pub(crate) fn read_document<S, T, R>(source: R, root: &'static str, what: &'static str) -> Result<T>
where
    R: BufRead,
    S: DeserializeSync<'static, RootRecordingReader<IoReader<R>>>,
    S::Error: std::error::Error + Send + Sync + 'static,
    T: TryFrom<S, Error = Error>,
{
    let mut reader = RootRecordingReader::new(IoReader::new(source));

    let raw = S::deserialize(&mut reader).map_err(|error| Error::Parse {
        what,
        path: None,
        source: Box::new(error),
    })?;

    ensure_root_is(&reader, root, what)?;

    T::try_from(raw)
}

/// Opens `path` and runs [`read_document`] on it, naming the file in any
/// error that has somewhere to put it (see `Error::with_path`).
pub(crate) fn read_document_at<S, T>(
    path: &Path,
    root: &'static str,
    what: &'static str,
) -> Result<T>
where
    S: DeserializeSync<'static, RootRecordingReader<IoReader<BufReader<File>>>>,
    S::Error: std::error::Error + Send + Sync + 'static,
    T: TryFrom<S, Error = Error>,
{
    let file = File::open(path).map_err(|source| Error::Open {
        path: path.to_path_buf(),
        source,
    })?;

    read_document::<S, T, _>(BufReader::new(file), root, what)
        .map_err(|error| error.with_path(path))
}

/// The whole write pipeline every format's `write_*_to` runs: convert `T`
/// (layer 2, domain) into the layer 1 schema type `S` via each format's
/// `schema_writer`, then serialize it to `sink` rooted at `<root>`.
///
/// `S` is generic-bound to `TryFrom<&'v T>` rather than `From<&'v T>`: every
/// format's conversion is fallible in the read direction (a lane's shape
/// might not parse), and while none of today's write directions can
/// actually fail — building a `String` from a [`uom`] quantity or an
/// already-validated enum always succeeds — a future one plausibly could
/// (a `SpreadType` this crate doesn't recognize, say), and `TryFrom` costs
/// nothing extra at call sites that never see an error either way.
///
/// Writes SUMO's usual XML declaration ahead of the root element:
/// [`SerializeSync::serialize`] only emits the element tree, not the
/// `<?xml ... ?>` prologue every `.net.xml`/`.rou.xml`/`.add.xml` this crate
/// has read starts with.
#[cfg(feature = "write")]
pub(crate) fn write_document<'v, S, T, W>(
    value: &'v T,
    root: &str,
    what: &'static str,
    sink: W,
) -> Result<()>
where
    W: std::io::Write,
    S: TryFrom<&'v T, Error = Error> + SerializeSync,
    <S as SerializeSync>::Error: std::error::Error + Send + Sync + 'static,
{
    let raw = S::try_from(value)?;
    let failed_to_write = |source| Error::Write {
        what,
        path: None,
        source,
    };

    let mut writer = Writer::new_with_indent(sink, b' ', 4);
    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
        .map_err(failed_to_write)?;
    raw.serialize(root, &mut writer)
        .map_err(|error| Error::Serialize {
            what,
            source: Box::new(error),
        })?;
    writer.get_mut().write_all(b"\n").map_err(failed_to_write)?;

    Ok(())
}

/// Creates `path` and runs [`write_document`] on it, naming the file in any
/// error that has somewhere to put it (see `Error::with_path`).
#[cfg(feature = "write")]
pub(crate) fn write_document_at<'v, S, T>(
    value: &'v T,
    root: &str,
    what: &'static str,
    path: &Path,
) -> Result<()>
where
    S: TryFrom<&'v T, Error = Error> + SerializeSync,
    <S as SerializeSync>::Error: std::error::Error + Send + Sync + 'static,
{
    let file = File::create(path).map_err(|source| Error::Create {
        path: path.to_path_buf(),
        source,
    })?;

    write_document::<S, T, _>(value, root, what, std::io::BufWriter::new(file))
        .map_err(|error| error.with_path(path))
}
