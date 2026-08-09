# sumo-types

Reads [SUMO](https://sumo.dlr.de/) files into well-typed Rust structs.

At the time of writing there is no other Rust crate that does this:
[`traci-rs`](https://crates.io/crates/traci-rs) speaks the TraCI protocol to
a *running* simulation, and SUMO's own `sumolib` is Python. This crate fills
the offline, file-reading gap.

```console
$ cargo add sumo-types                            # `net` only (default)
$ cargo add sumo-types --features routes          # `net` + `routes`
$ cargo add sumo-types --features additional      # `net` + `additional`
```

Requires Rust 1.86 or newer.

```rust,no_run
use sumo_types::domain::JunctionKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let network = sumo_types::read_network(std::path::Path::new("city.net.xml"))?;

    for junction in &network.junctions {
        if junction.kind == JunctionKind::TrafficLight {
            println!("{} has {} incoming lanes", junction.id, junction.incoming_lanes.len());
        }
    }

    Ok(())
}
```

## Formats

SUMO has one file format per Cargo feature. Two are implemented:

| Feature | Format | Public API |
|---|---|---|
| `net` (default) | `.net.xml` (`netconvert` output) | `sumo_types::domain`, `sumo_types::read_network` |
| `routes` | `.rou.xml` (traffic demand) | `sumo_types::routes` |
| `additional` | `.add.xml` (E1/E2/E3 detectors) | `sumo_types::additional` |

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let routes = sumo_types::routes::read_routes(std::path::Path::new("city.rou.xml"))?;
    for vehicle in &routes.vehicles {
        println!("{} departs at {:?}", vehicle.id, vehicle.depart);
    }

    Ok(())
}
```

```rust,no_run
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let additional = sumo_types::additional::read_additional(
        std::path::Path::new("city.add.xml"),
    )?;
    for detector in &additional.entry_exit_detectors {
        println!("{} has {} entries", detector.id, detector.entries.len());
    }

    Ok(())
}
```

All three examples above are compiled as doctests (see `ReadmeExamples`
in `src/lib.rs`), so they can't drift from the real API.

`net`'s API sits at the crate root (`sumo_types::domain::Network`,
`sumo_types::read_network`) because it was this crate's first, only format,
and that path is kept stable now. `routes`, added later, is namespaced at
`sumo_types::routes` instead — both to avoid colliding with `net`'s type
names (a route's edges and a network's edges are both just ids, but
`domain::EdgeId` and `routes::domain::EdgeRef` are unrelated Rust types) and
so the path a type is imported from tells you which SUMO format it belongs
to. Internally, `net`'s files live under `src/net/` and are re-exported at
the crate root — a module's file location and its public path are
independent in Rust, so nesting the files didn't have to mean nesting the
public API too.

## How it works

SUMO ships XML Schema (XSD) definitions for its file formats; they're
vendored under `xsd/`. At build time, `build.rs` feeds the XSD of every
active feature to [`xsd-parser`](https://crates.io/crates/xsd-parser) — in a
single generation pass, so formats that share common schemas (like
`types/base.xsd`, which defines `positionType`, `boolType`, `colorType`,
...) get one compatible copy of those types instead of each its own —
patching a few SUMO-specific XSD quirks along the way (unresolved DTD
entities, type names that collide with XSD primitives, and a custom naming
strategy that keeps case-sensitive enumeration values like `state="M"` vs.
`state="m"` from colliding into the same Rust identifier). See
`FEATURE_SCHEMAS` in `build.rs`.

Each format has 2 layers of types, and a conversion step between them. For
`net`, a `.net.xml` file becomes a `Network` like this (`routes` mirrors it,
`schema::RoutesType` becoming a `routes::domain::Routes`):

```text
.net.xml --(xsd-parser, at build time)--> schema --(schema_mapper)--> domain
```

- **`schema`** (layer 1; generated, see `build.rs`) — an almost literal
  mirror of the SUMO XSDs. **Private**, not just discouraged: see below.
  Shared by every active feature, so `schema::NetType` and
  `schema::RoutesType` live in the same module when both `net` and `routes`
  are enabled.
- **`schema_mapper`** (`src/net/schema_mapper.rs`, `src/routes/schema_mapper.rs`)
  — converts layer 1 into layer 2, interpreting SUMO's text-encoded
  positions, shapes, boundaries, unions, and enumerations along the way. For
  example, `schema::LaneType::shape` is a raw `String`
  (`"0.00,0.00 100.00,0.00"`); `net`'s `schema_mapper` turns it into a
  `domain::Shape` (a `Vec<domain::Point>`).
- **`domain`** (layer 2; `src/net/domain.rs`, `src/routes/domain.rs`) — each
  format's own types (`Network`, `Edge`, `Lane`, ... for `net`; `Route`,
  `VehicleType`, `Vehicle`, ... for `routes`), independent of SUMO/XSD and of
  each other. This is what consumers build on.

`read_network` / `routes::read_routes` run the whole pipeline for their
format — deserializing into layer 1, then converting via `schema_mapper`
into layer 2 — so consumers only ever see `domain` types, and never have to
name a generated schema type or depend on `xsd-parser-types` themselves.

Layer 1 being private is a deliberate semver decision, not tidiness. The
generated types implement `xsd-parser-types`' traits (`WithDeserializer`,
`DeserializeBytes`, ...) in 173 impls, so a public `schema` would put that
crate into this one's public API — and since a trait is identified by the
version of the crate defining it, `WithDeserializer` from 0.2 and from 0.3
are different traits. Bumping the dependency would then break every
consumer built on layer 1, forcing a breaking release over a change that
has nothing to do with this crate's own API. Layer 2 never mentions
`xsd-parser-types`, so keeping layer 1 private is what lets that dependency
stay an implementation detail. (`uom` is the opposite case, and *is*
re-exported: `domain::Lane::length` is a `Length`, so it is public API by
design rather than by accident.)

Because layer 1 is regenerated from the XSDs on every build, keeping up with
a new SUMO release is usually a matter of dropping in the updated `xsd/`
directory. What is hand-maintained is only the handful of patches in
`build.rs` and each format's own `schema_mapper.rs`.

### What `routes` covers

Only the building blocks of a traffic demand file are modeled: `Route`,
`VehicleType` (SUMO's `vType`), and `Vehicle`. `routesType` also allows
`flow`, `trip`, `person`, `personFlow`, `container`, `containerFlow`,
`interval`, `include`, `vTypeDistribution`, and `routeDistribution`
elements — `read_routes` silently drops them rather than erroring, same as
how `net`'s `domain::Network` doesn't cover every attribute `net_file.xsd`
allows either. See the doc comments on `routes::domain`'s types for exactly
what's mapped from each SUMO element.

### What `additional` covers

Only the detector definitions: `E1Detector` (induction loop), `E2Detector`
(lane area) and `E3Detector` (multi-entry / multi-exit), plus the
`detEntry`/`detExit` gates of the last one. `additionalType` allows about
forty more element kinds — `busStop`, `chargingStation`, `parkingArea`,
`rerouter`, `calibrator`, `variableSpeedSign`, `WAUT`, `poly`, `poi`, and
the whole of `routesType` over again — and `read_additional` silently drops
them, same as `routes` drops the parts of its own format it doesn't model.

SUMO spells each detector two ways (`e1Detector`/`inductionLoop`,
`e2Detector`/`laneAreaDetector`, `e3Detector`/`entryExitDetector`): two
element names bound to one XSD type. Both spellings land in the same field.

This is a *reader*, like the rest of the crate. It parses a `.add.xml`
someone else wrote; it does not emit one.

## Units

Physical quantities are [`uom`](https://crates.io/crates/uom) types
(`Length`, `Velocity`, `Time`) rather than bare numbers, so a lane's length
can't be silently mixed up with its speed and unit conversion is a
`.get::<_>()` call instead of a magic number. `uom` is re-exported as
`sumo_types::uom` so you can build values with the exact version this crate
was compiled against.

## Adding another format

Most of the work is mechanical, because the generation layer is already
format-agnostic: two of the three SUMO-specific quirks `build.rs` patches
around live in `types/base.xsd`, which 46 of the 75 vendored schemas
include, and apply to every schema alike. The third, `PER_FILE_PATCHES`,
is keyed by file name for quirks specific to one schema. `routes` and
`additional` were both added this way; for another one:

1. Add a feature for it in `Cargo.toml`.
2. Add its entry XSD to `FEATURE_SCHEMAS` in `build.rs`.
3. Add its own `domain`, `schema_mapper`, and `reader` files (under
   `src/<format>/`, mirroring `src/net/` or `src/routes/`) — a `Route` is
   not an `Edge`. Decide whether to flatten its API to the crate root (like
   `net`) or namespace it under its own module (like `routes`); the latter
   is simpler once more than one non-`net` format exists, since only one of
   them can occupy the crate root's `domain`/`read_*` names.
4. Have that reader call `xml::ensure_root_is` after deserializing.
   xsd-parser generates deserializers for XSD *types*, not elements, so
   nothing else checks the root element's name — and since most SUMO
   schemas make their content optional, an unrelated document otherwise
   parses into an empty, plausible-looking value instead of an error.
5. Anything a crate-level (`//!`) doc comment mentions has to be written as
   a plain code span, not an intra-doc link, and any example in one needs
   `ignore` rather than `no_run` unless the format is a default feature.
   Those comments aren't attached to a single item, so they can't be
   `#[cfg]`-gated: they are rendered and compiled under *every* feature
   combination, and a link to a feature-gated item is a broken link in each
   build that doesn't enable it. Put the real, runnable example on the
   reader function's own doc comment, where the feature gate applies
   naturally, and let the README doctests (`ReadmeExamples` in `src/lib.rs`)
   cover the cross-format story.

Some schemas need a `PER_FILE_PATCHES` entry before they generate at all.
`additional_file.xsd` needed two, both documented at that constant in
`build.rs`:

- Untouched it fails with `UnknownType(fileOptionType)`. The file never
  mentions `fileOptionType`; it drags it in through `types/metadata.xsd`,
  which includes the 13 `*ConfigurationType.xsd` files describing every
  SUMO tool's command-line options. All `additional_file.xsd` wants from
  that subtree is one optional `<metadata>` provenance element, so the
  patch drops both and the include graph collapses to the same
  `route.xsd` + `taz.xsd` + `base.xsd` shape the other two formats
  already generate from. The cost is that `<metadata>` can't be read.
- xsd-parser names the type behind an anonymous `xsd:choice` after a
  global counter over everything generated so far, so `e3Detector`'s
  `detEntry`/`detExit` choice came out as `E3DetectorContent75Type` with
  every format enabled but `E3DetectorContent70Type` with only
  `additional` — a type name that shifts with the consumer's feature
  selection, which `schema_mapper` cannot write down. The patch hoists
  the choice into a named `xsd:group`, so the name derives from the group
  (`E3DetectorDetGateGroupType`) and is stable everywhere. Watch for this
  in any new schema with an inline `xsd:choice`.

Both patches are matched against literal text from Eclipse SUMO's
schemas and fail the build if they stop matching, which is what should
happen when `xsd/` is re-vendored from a newer SUMO.

## Scope of the published package

Only the schemas the implemented features need (`net_file.xsd`,
`routes_file.xsd`, `additional_file.xsd`, `types/base.xsd`,
`types/route.xsd`, `types/taz.xsd`) are
published to crates.io — see `include` in `Cargo.toml`. The remaining 69
vendored schemas (detector outputs, tool configurations, ...) stay in the
git repository so that adding a reader for another format later doesn't
mean re-vendoring them; they'd be added to `include` alongside that
format's feature.

## License

This crate's own code (`src/`, `build.rs`) is licensed under either of
[Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.

The schemas under `xsd/` are third-party material redistributed verbatim
from Eclipse SUMO under [EPL-2.0](LICENSE-EPL), and are *not* covered by
that dual license — see [NOTICE](NOTICE). Because the published package
contains both, its crates.io `license` field is the combined expression
`(MIT OR Apache-2.0) AND EPL-2.0`, which is what license scanners need to
see; it describes the tarball, not just the Rust source.
