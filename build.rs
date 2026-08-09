use anyhow::{Context, Result, bail};
use inflector::Inflector;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::AtomicUsize};
use xsd_parser::{
    Config, Ident2, Name, TypeIdent,
    config::Schema,
    generate,
    models::{
        NameBuilder as DefaultNameBuilder, format_ident, format_unknown_variant, make_type_name,
        meta::MetaType,
    },
    traits::{NameBuilder, Naming},
};

const ORIGINAL_XSD_DIR: &str = "xsd";
const OUTPUT_FILE_NAME: &str = "generated_schema.rs";

/// Maps each of this crate's `[features]` (see `Cargo.toml`) to the XSD its
/// reader is generated from. Adding a new SUMO file format is meant to be
/// mostly a matter of adding an entry here, a matching feature, and that
/// format's own `domain`/`schema_mapper` module — the XSD patching below and
/// the `SumoNaming` strategy are already format-agnostic.
///
/// Every active feature's schema is generated in a single `generate()` call
/// (see `main`), not one call per feature: `xsd-parser` scopes type
/// resolution per call, so generating separately would give each format its
/// own incompatible copy of the primitives common schemas like
/// `types/base.xsd` define (`positionType`, `boolType`, ...) instead of one
/// shared `schema` module.
///
/// Not every SUMO schema generates as-is; `additional_file.xsd` needs the
/// targeted patch in [`PER_FILE_PATCHES`] first.
const FEATURE_SCHEMAS: &[(&str, &str)] = &[
    ("net", "net_file.xsd"),
    ("routes", "routes_file.xsd"),
    ("additional", "additional_file.xsd"),
];

/// Cargo sets `CARGO_FEATURE_<NAME>` (uppercased, `-` -> `_`) for every
/// enabled feature of the crate the build script belongs to.
fn feature_enabled(name: &str) -> bool {
    env::var(format!(
        "CARGO_FEATURE_{}",
        name.to_uppercase().replace('-', "_")
    ))
    .is_ok()
}

/// Some SUMO xsd files (types/base.xsd) declare DTD entity constants in
/// their own <!DOCTYPE ...> (e.g. <!ENTITY FloatPattern "[-+]?...">) and use
/// them inside `xsd:pattern` patterns (`&FloatPattern;`). The XML parser used by
/// xsd-parser does not resolve the DOCTYPE's internal subset, so those
/// references make parsing fail if left as-is.
///
/// Unlike neutralizing them (replacing them with a wildcard), here we
/// resolve them to their real value: since they're declared and used within
/// the same file, the substitution is textual and doesn't lose precision
/// from the original pattern. Standard XML entities (&amp; &lt; &gt; &apos;
/// &quot;) are left untouched.
fn resolve_dtd_entities(content: &str) -> String {
    let entities = parse_dtd_entities(content);
    if entities.is_empty() {
        return content.to_string();
    }

    let mut segments = content.split('&');
    let first = segments.next().unwrap_or_default().to_string();

    segments.fold(first, |mut resolved, segment| {
        resolved.push_str(&resolve_entity_reference(segment, &entities));
        resolved
    })
}

/// Extracts the `(name, value)` pairs from the `<!ENTITY ...>` declarations
/// present in `content`'s DOCTYPE internal subset.
fn parse_dtd_entities(content: &str) -> Vec<(&str, &str)> {
    content
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("<!ENTITY ")?.strip_suffix('>')?;
            let (name, value) = rest.split_once(char::is_whitespace)?;
            let value = value.trim().strip_prefix('"')?.strip_suffix('"')?;
            Some((name.trim(), value))
        })
        .collect()
}

/// Resolves the piece of text that follows an `&` (the result of splitting
/// the original content on that character): if it starts with a known
/// entity reference, it's replaced with its value; otherwise, the `&` is
/// restored.
fn resolve_entity_reference(segment: &str, entities: &[(&str, &str)]) -> String {
    let Some((name, rest)) = segment.split_once(';') else {
        return format!("&{segment}");
    };

    let value = match name {
        "amp" | "lt" | "gt" | "apos" | "quot" => return format!("&{name};{rest}"),
        _ => entities
            .iter()
            .find_map(|(n, v)| (*n == name).then_some(*v)),
    };

    match value {
        Some(value) => format!("{value}{rest}"),
        None => format!("&{segment}"),
    }
}

/// xsd-parser derives Rust type names from XSD primitive types
/// (xsd:float -> `FloatType`, xsd:time -> `TimeType`, xsd:ID -> `IdType`, ...).
/// SUMO defines its own simpleType "floatType", "timeType" and "idType" in
/// base.xsd, which, once capitalized, collide exactly with those generated
/// primitive names, and xsd-parser doesn't disambiguate them (E0428: name
/// defined twice). We rename them so they don't collide; this only affects
/// the generated `schema` layer, not our own code.
const RENAMED_TYPES: &[(&str, &str)] = &[
    (r#""idType""#, r#""sumoIdType""#),
    (r#""floatType""#, r#""sumoFloatType""#),
    (r#""timeType""#, r#""sumoTimeType""#),
];

fn rename_colliding_types(content: &str) -> String {
    RENAMED_TYPES
        .iter()
        .fold(content.to_string(), |content, (from, to)| {
            content.replace(from, to)
        })
}

/// Patches that apply to one specific schema rather than to all of them:
/// `file name -> (literal text to find, replacement)`.
///
/// `additional_file.xsd` fails to generate untouched, with
/// `UnknownType(fileOptionType)`. The cause is not the file itself — it
/// never mentions `fileOptionType` — but its `types/metadata.xsd` include:
/// that schema pulls in the 13 `*ConfigurationType.xsd` files describing
/// every SUMO tool's command-line options (netconvert, duarouter,
/// polyconvert, ...), and `fileOptionType` lives in that graph. All
/// `additional_file.xsd` actually wants from it is one optional
/// `<metadata>` child of `<additional>`, carrying the provenance block SUMO
/// writes (which tool and version produced the file).
///
/// Dropping the include and that one `minOccurs="0"` element collapses the
/// graph to `route.xsd` + `taz.xsd` + `base.xsd` — the same shape
/// `net_file.xsd` and `routes_file.xsd` already generate from — and the
/// whole schema comes out clean. The cost is that `<metadata>` can't be
/// read; nothing this crate models needs it.
///
/// The second patch names an anonymous `xsd:choice`. xsd-parser derives a
/// name for such a type from a *global counter over everything it has
/// generated so far*, so the same choice came out as
/// `E3DetectorContent75Type` with all three formats enabled but
/// `E3DetectorContent70Type` with only `additional` — the ordinal shifts
/// with the active feature set, and `schema_mapper` can't name a type whose
/// name depends on which features the consumer picked. Hoisting the choice
/// into a named `xsd:group` makes xsd-parser derive the name from the group
/// instead (`E3DetectorDetGateGroupType`), stable across every
/// combination. The group
/// is a pure refactor of the XSD: `xsd:group` is an inlined definition, so
/// the document shape it accepts is unchanged.
///
/// Keyed by file name on purpose: [`patch_xsd`] runs over all 75 vendored
/// schemas, and 46 of them include `types/base.xsd` while several include
/// `types/metadata.xsd`. Applying this blindly would silently reshape
/// schemas that have nothing to do with `.add.xml`.
const PER_FILE_PATCHES: &[(&str, &[(&str, &str)])] = &[(
    "additional_file.xsd",
    &[
        // Cut the whole `types/metadata.xsd` subtree loose ...
        (
            "    <xsd:include schemaLocation=\"types/metadata.xsd\"/>\n",
            "",
        ),
        // ... along with the single optional element that needed it.
        (
            "            <xsd:element name=\"metadata\" type=\"metadataType\" minOccurs=\"0\" maxOccurs=\"1\"/>\n",
            "",
        ),
        // Give the detEntry/detExit choice a name of its own.
        (
            "            <xsd:choice minOccurs=\"2\" maxOccurs=\"unbounded\">\n\
             \x20               <xsd:element name=\"detEntry\" type=\"detEntryExitType\" minOccurs=\"1\" maxOccurs=\"unbounded\"/>\n\
             \x20               <xsd:element name=\"detExit\" type=\"detEntryExitType\" minOccurs=\"1\" maxOccurs=\"unbounded\"/>\n\
             \x20           </xsd:choice>\n",
            "            <xsd:group ref=\"detGateGroup\" minOccurs=\"2\" maxOccurs=\"unbounded\"/>\n",
        ),
        (
            "    <xsd:complexType name=\"e3DetectorType\">\n",
            "    <xsd:group name=\"detGateGroup\">\n\
             \x20       <xsd:choice>\n\
             \x20           <xsd:element name=\"detEntry\" type=\"detEntryExitType\" minOccurs=\"1\" maxOccurs=\"unbounded\"/>\n\
             \x20           <xsd:element name=\"detExit\" type=\"detEntryExitType\" minOccurs=\"1\" maxOccurs=\"unbounded\"/>\n\
             \x20       </xsd:choice>\n\
             \x20   </xsd:group>\n\
             \n\
             \x20   <xsd:complexType name=\"e3DetectorType\">\n",
        ),
    ],
)];

/// Applies the [`PER_FILE_PATCHES`] registered for `file_name`. A no-op for
/// any schema with no entry there, which is all but one of them.
///
/// Fails if a pattern isn't found rather than skipping it: these patches
/// are matched against literal text from Eclipse SUMO's schemas, so
/// re-vendoring an updated `xsd/` is exactly when one would stop applying.
/// Silently skipping would surface much later as an opaque xsd-parser
/// error, or — worse, for the naming patch — as generated code that still
/// compiles but under a different type name.
fn apply_per_file_patches(file_name: &str, content: &str) -> Result<String> {
    let Some((_, patches)) = PER_FILE_PATCHES.iter().find(|(name, _)| *name == file_name) else {
        return Ok(content.to_string());
    };

    patches
        .iter()
        .try_fold(content.to_string(), |content, (from, to)| {
            if !content.contains(from) {
                bail!(
                    "patch for {file_name} no longer matches; the vendored schema must have \
                     changed. Expected to find:\n{from}"
                );
            }

            Ok(content.replace(from, to))
        })
}

/// Composition of the text patches applied to each `.xsd` before handing it
/// to xsd-parser. `file_name` selects the [`PER_FILE_PATCHES`]; the other
/// two patches are format-agnostic and apply to every schema.
fn patch_xsd(file_name: &str, content: &str) -> Result<String> {
    let content = apply_per_file_patches(file_name, content)?;
    Ok(rename_colliding_types(&resolve_dtd_entities(&content)))
}

/// Recursively copies `src` into `dst`, patching (see [`patch_xsd`]) any
/// `.xsd` file it finds along the way.
fn copy_and_patch_schemas(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    fs::read_dir(src)?.try_for_each(|entry| {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            return copy_and_patch_schemas(&path, &dest_path);
        }

        if path.extension().is_some_and(|ext| ext == "xsd") {
            let content = fs::read_to_string(&path)?;
            let file_name = entry.file_name();
            let patched = patch_xsd(&file_name.to_string_lossy(), &content)?;
            fs::write(&dest_path, patched)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }

        Ok(())
    })
}

/// Naming strategy passed to xsd-parser (`Config::with_naming`) for
/// types/modules/fields/constants: identical to the default one (unifies to
/// `PascalCase`/`snake_case`/`SCREAMING_SNAKE_CASE`), but with
/// [`format_variant_name`] replaced by a version that doesn't lose
/// information (see its documentation).
///
/// The `unify` logic is duplicated here because xsd-parser doesn't expose it
/// publicly (`unify_string` is private to the crate); this is a faithful
/// copy of its default implementation.
#[derive(Debug, Clone, Default)]
struct SumoNaming(Arc<AtomicUsize>);

impl Naming for SumoNaming {
    fn clone_boxed(&self) -> Box<dyn Naming> {
        Box::new(self.clone())
    }

    fn builder(&self) -> Box<dyn NameBuilder> {
        Box::new(DefaultNameBuilder::new(
            self.0.clone(),
            Box::new(self.clone()),
        ))
    }

    fn unify(&self, s: &str) -> String {
        unify(s)
    }

    fn make_type_name(&self, postfixes: &[String], ty: &MetaType, ident: &TypeIdent) -> Name {
        make_type_name(self, postfixes, ty, ident)
    }

    fn make_unknown_variant(&self, id: usize) -> Ident2 {
        format_unknown_variant(id)
    }

    fn format_module_name(&self, s: &str) -> String {
        format_ident(self.unify(s).to_snake_case())
    }

    fn format_type_name(&self, s: &str) -> String {
        format_ident(self.unify(s))
    }

    fn format_field_name(&self, s: &str) -> String {
        format_ident(self.unify(s).to_snake_case())
    }

    fn format_variant_name(&self, s: &str) -> String {
        format_variant_name(s)
    }

    fn format_constant_name(&self, s: &str) -> String {
        format_ident(self.unify(s).to_screaming_snake_case())
    }
}

/// Faithful copy of the `PascalCase` normalization used by xsd-parser's
/// default `Naming` for types/modules/fields/constants (its real
/// implementation, `unify_string`, is not public).
fn unify(s: &str) -> String {
    let mut done = true;
    let unified = s.replace(
        |c: char| {
            let replace = !c.is_alphanumeric();
            if c != '_' && !replace {
                done = false;
            }
            c != '_' && replace
        },
        "_",
    );

    if done {
        unified
    } else {
        unified.to_screaming_snake_case().to_pascal_case()
    }
}

/// Builds an enum variant name from the XSD value `s`.
///
/// xsd-parser's default `Naming` unifies everything to `PascalCase`, which is
/// case-insensitive: XSD values that only differ by case (e.g. the
/// single-character codes `net_file.xsd` uses for `state="M"/"m"` or
/// `dir="s"/"t"/"T"`, or "true"/"True" in `boolType`) collapse into the same
/// Rust identifier, which xsd-parser neither detects nor disambiguates
/// (duplicate enum variants -> compile error).
///
/// Here, instead, we preserve all the information from the original value:
/// alphanumeric characters are kept as-is (with their original case), and
/// each non-alphanumeric symbol is translated to a distinct word (instead of
/// collapsing them all to "_", which would also produce the reserved `_`
/// identifier for single-symbol values like "-").
fn format_variant_name(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_string()
            } else {
                describe_symbol(c)
            }
        })
        .collect();

    format_ident(if sanitized.is_empty() {
        "Empty".to_string()
    } else {
        sanitized
    })
}

/// Gives a readable, distinct name to a non-alphanumeric character, so that
/// two different symbols (e.g. "-" and "=" in `connectionType/@state`) never
/// collapse into the same identifier.
fn describe_symbol(c: char) -> String {
    match c {
        '-' => "Dash".to_string(),
        '=' => "Eq".to_string(),
        '_' => "_".to_string(),
        other => format!("U{:x}", other as u32),
    }
}

/// Formats the generated code so that rustc diagnostics pointing into
/// `schema` have real line numbers to point at: xsd-parser hands back a
/// `TokenStream`, whose `to_string()` is a single multi-megabyte line.
///
/// Falls back to the unformatted source rather than failing the build — a
/// pretty-printing problem is a readability problem, not a correctness one,
/// and `syn` failing to re-parse xsd-parser's own output would be its bug,
/// not something a consumer of this crate can act on.
fn pretty_print(code: &str) -> String {
    match syn::parse_file(code) {
        Ok(parsed) => prettyplease::unparse(&parsed),
        Err(error) => {
            println!("cargo:warning=could not pretty-print the generated schema: {error}");
            code.to_string()
        }
    }
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed={ORIGINAL_XSD_DIR}");

    let out_dir =
        PathBuf::from(env::var("OUT_DIR").context("OUT_DIR environment variable is not set")?);
    let dest_path = out_dir.join(OUTPUT_FILE_NAME);

    let active_schemas: Vec<&str> = FEATURE_SCHEMAS
        .iter()
        .filter(|(feature, _)| feature_enabled(feature))
        .map(|(_, xsd)| *xsd)
        .collect();

    // With no format feature there is no schema to generate, but failing
    // here would surface as an opaque "custom build command failed". Write
    // an empty module instead and let the `compile_error!` in `lib.rs`
    // report it as an ordinary rustc diagnostic naming the missing feature.
    if active_schemas.is_empty() {
        fs::write(&dest_path, "")
            .with_context(|| format!("Failed to write empty schema to {}", dest_path.display()))?;
        return Ok(());
    }

    let patched_xsd_dir = out_dir.join("patched_xsd");
    copy_and_patch_schemas(Path::new(ORIGINAL_XSD_DIR), &patched_xsd_dir)
        .context("Failed to patch and copy XSD schemas")?;

    let mut config = Config::default()
        .with_naming(SumoNaming::default())
        .with_quick_xml_deserialize();
    config.parser.schemas = active_schemas
        .iter()
        .map(|xsd| Schema::File(patched_xsd_dir.join(xsd)))
        .collect();

    let code = generate(config)
        .map_err(|e| anyhow::anyhow!("Error generating code from XSD schema: {e:?}"))?;

    fs::write(&dest_path, pretty_print(&code.to_string()))
        .with_context(|| format!("Failed to write generated file to {}", dest_path.display()))?;

    Ok(())
}
