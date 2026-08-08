# sumo-types

Reads [SUMO](https://sumo.dlr.de/) files into well-typed Rust structs.

At the time of writing there is no other Rust crate that does this:
[`traci-rs`](https://crates.io/crates/traci-rs) speaks the TraCI protocol to
a *running* simulation, and SUMO's own `sumolib` is Python. This crate fills
the offline, file-reading gap.

```rust
let network = sumo_types::read_network(std::path::Path::new("city.net.xml"))?;

for junction in &network.junctions {
    if junction.kind == sumo_types::domain::JunctionKind::TrafficLight {
        println!("{} has {} incoming lanes", junction.id, junction.incoming_lanes.len());
    }
}
```

## Formats

SUMO has one file format per Cargo feature. Two are implemented:

| Feature | Format | Public API |
|---|---|---|
| `net` (default) | `.net.xml` (`netconvert` output) | `sumo_types::domain`, `sumo_types::read_network` |
| `routes` | `.rou.xml` (traffic demand) | `sumo_types::routes` |

```rust
let routes = sumo_types::routes::read_routes(std::path::Path::new("city.rou.xml"))?;
for vehicle in &routes.vehicles {
    println!("{} departs at {:?}", vehicle.id, vehicle.depart);
}
```

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
  mirror of the SUMO XSDs. Not meant to be used directly. Shared by every
  active feature: `schema::NetType` and `schema::RoutesType` live in the
  same module when both `net` and `routes` are enabled.
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

## Units

Physical quantities are [`uom`](https://crates.io/crates/uom) types
(`Length`, `Velocity`, `Time`) rather than bare numbers, so a lane's length
can't be silently mixed up with its speed and unit conversion is a
`.get::<_>()` call instead of a magic number. `uom` is re-exported as
`sumo_types::uom` so you can build values with the exact version this crate
was compiled against.

## Adding another format

Most of the work is mechanical, because the generation layer is already
format-agnostic — the two SUMO-specific quirks `build.rs` patches around
both live in `types/base.xsd`, which 46 of the 75 vendored schemas include.
`routes` was added this way; for another one:

1. Add a feature for it in `Cargo.toml`.
2. Add its entry XSD to `FEATURE_SCHEMAS` in `build.rs`.
3. Add its own `domain`, `schema_mapper`, and `reader` files (under
   `src/<format>/`, mirroring `src/net/` or `src/routes/`) — a `Route` is
   not an `Edge`. Decide whether to flatten its API to the crate root (like
   `net`) or namespace it under its own module (like `routes`); the latter
   is simpler once more than one non-`net` format exists, since only one of
   them can occupy the crate root's `domain`/`read_*` names.
4. Any crate-level (`//!`) doc examples that reference the new format's API
   need `ignore` instead of `no_run` unless the format is a default feature
   — those doc comments aren't attached to a single item, so they can't be
   `#[cfg]`-gated, and have to compile under every feature combination.
   Put the real, runnable example on the reader function's own doc comment
   instead, where it's naturally gated by that format's feature.

Not every SUMO schema can be added this way, at least not with xsd-parser
1.5.2: `additional_file.xsd` fails generation
(`UnknownType(fileOptionType)`) because its include graph reaches
`types/base.xsd` via three different paths (directly, and via
`types/route.xsd` and `types/taz.xsd`), and xsd-parser doesn't resolve that
diamond correctly. `net_file.xsd` and `routes_file.xsd` each avoid this
because they only reach `types/base.xsd` by a single path (through
`types/taz.xsd` and `types/route.xsd` respectively). Confirmed by generating
`types/base.xsd` and the file that needs it as the only two schema roots,
which still fails the same way — so it isn't fixable by including
`base.xsd` more explicitly.

## Scope of the published package

Only the schemas the implemented features need (`net_file.xsd`,
`routes_file.xsd`, `types/base.xsd`, `types/route.xsd`, `types/taz.xsd`) are
published to crates.io — see `include` in `Cargo.toml`. The remaining 70
vendored schemas (detector outputs, tool configurations, ...) stay in the
git repository so that adding a reader for another format later doesn't
mean re-vendoring them; they'd be added to `include` alongside that
format's feature.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. The schemas under `xsd/` are
third-party material redistributed from Eclipse SUMO under EPL-2.0, not
covered by this crate's license — see [NOTICE](NOTICE).
