//! Shared XML plumbing for every format's reader.
//!
//! xsd-parser generates deserializers for XSD *types*, not for elements, so
//! `schema::NetType::deserialize` will happily consume a document whose root
//! is named anything at all as long as its children fit the type. That makes
//! it possible to hand a `.rou.xml` (or any unrelated XML) to
//! [`crate::read_network`] and get back a plausible-looking, empty
//! `Network`. [`RootRecordingReader`] closes that gap.

use anyhow::Context;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use xsd_parser_types::quick_xml::{
    DeserializeSync, Error, Event, IoReader, XmlReader, XmlReaderSync,
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
    fn extend_error(&self, error: Error) -> Error {
        self.inner.extend_error(error)
    }
}

impl<'a, R: XmlReaderSync<'a>> XmlReaderSync<'a> for RootRecordingReader<R> {
    fn read_event(&mut self) -> Result<Event<'a>, Error> {
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
    expected: &str,
    what: &str,
) -> anyhow::Result<()> {
    match reader.root_name() {
        Some(name) if name == expected.as_bytes() => Ok(()),
        Some(name) => anyhow::bail!(
            "not a {what}: expected a <{expected}> root element, found <{}>",
            String::from_utf8_lossy(name)
        ),
        None => anyhow::bail!("not a {what}: the document has no root element"),
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
pub(crate) fn read_document<S, T, R>(source: R, root: &str, what: &str) -> anyhow::Result<T>
where
    R: BufRead,
    S: DeserializeSync<'static, RootRecordingReader<IoReader<R>>>,
    S::Error: Display,
    T: TryFrom<S, Error = anyhow::Error>,
{
    let mut reader = RootRecordingReader::new(IoReader::new(source));

    let raw = S::deserialize(&mut reader)
        .map_err(|error| anyhow::anyhow!("failed to parse {what}: {error}"))?;

    ensure_root_is(&reader, root, what)?;

    T::try_from(raw)
}

/// Opens `path` and runs [`read_document`] on it, naming the file in any
/// error it produces.
pub(crate) fn read_document_at<S, T>(path: &Path, root: &str, what: &str) -> anyhow::Result<T>
where
    S: DeserializeSync<'static, RootRecordingReader<IoReader<BufReader<File>>>>,
    S::Error: Display,
    T: TryFrom<S, Error = anyhow::Error>,
{
    let file = File::open(path)
        .with_context(|| format!("Could not open input file: {}", path.display()))?;

    read_document::<S, T, _>(BufReader::new(file), root, what)
        .with_context(|| format!("invalid {what} in {}", path.display()))
}
