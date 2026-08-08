# sumo-net

Reads [SUMO](https://sumo.dlr.de/) road networks (`.net.xml`, as produced by
`netconvert`) into well-typed Rust structs.

It's standalone on purpose — nothing here knows about any particular
consumer (its first one is Verdi's `waiting_zones`, which depends on it with
`path = "../sumo-net"`).

At the time of writing there is no other Rust crate that parses `.net.xml`:
[`traci-rs`](https://crates.io/crates/traci-rs) speaks the TraCI protocol to
a *running* simulation, and SUMO's own `sumolib` is Python. This crate fills
the offline, file-reading gap.

```rust
let network = sumo_net::read_network(std::path::Path::new("city.net.xml"))?;

for junction in &network.junctions {
    if junction.kind == sumo_net::domain::JunctionKind::TrafficLight {
        println!("{} has {} incoming lanes", junction.id, junction.incoming_lanes.len());
    }
}
```

## How it works

SUMO ships XML Schema (XSD) definitions for its file formats; they're
vendored under `xsd/`. At build time, `build.rs` feeds `xsd/net_file.xsd` to
[`xsd-parser`](https://crates.io/crates/xsd-parser) to generate matching Rust
types, patching a few SUMO-specific XSD quirks along the way (unresolved DTD
entities, type names that collide with XSD primitives, and a custom naming
strategy that keeps case-sensitive enumeration values like `state="M"` vs.
`state="m"` from colliding into the same Rust identifier).

The crate follows a 3-layer model:

1. **`schema`** (generated, see `build.rs`) — an almost literal mirror of the
   SUMO XSDs. Not meant to be used directly outside of the conversion layer.
2. **`domain`** (`src/domain.rs`) — the crate's own types (`Network`, `Edge`,
   `Lane`, `Junction`, `Connection`, ...), independent of SUMO/XSD. This is
   what consumers build on.
3. **`schema_mapper`** (`src/schema_mapper.rs`) — converts layer 1 into layer
   2, interpreting SUMO's text-encoded positions, shapes, boundaries, and
   enumerations along the way.

`src/reader.rs` ties layers 1 and 3 together into `read_network`, so
consumers never have to name a generated type or depend on
`xsd-parser-types` themselves.

Because layer 1 is regenerated from the XSDs on every build, keeping up with
a new SUMO release is usually a matter of dropping in the updated `xsd/`
directory. What is hand-maintained is only the handful of patches in
`build.rs` and the mapping in `schema_mapper.rs`.

## Units

Physical quantities are [`uom`](https://crates.io/crates/uom) types
(`Length`, `Velocity`) rather than bare `f64`s, so a lane's length can't be
silently mixed up with its speed and unit conversion is a `.get::<_>()` call
instead of a magic number. `uom` is re-exported as `sumo_net::uom` so you can
build values with the exact version this crate was compiled against.

## Scope

Only `.net.xml` is parsed today. `net_file.xsd` pulls in just two other
schemas (`types/base.xsd` and `types/taz.xsd`), and those three are the only
ones published — see `include` in `Cargo.toml`. The remaining 72 schemas are
kept in the repository (they describe routes, detector outputs, tool
configurations, ...) so that adding a reader for another SUMO format doesn't
mean re-vendoring them.

Adding one is mostly mechanical, because the two SUMO-specific quirks
`build.rs` patches around both live in `types/base.xsd`, which 46 of the 75
schemas include — the generation layer is already format-agnostic. What each
new format does need is its own layer 2 and layer 3 (a `Route` is not an
`Edge`), though the primitive conversions in `schema_mapper.rs` (`Point`,
`Shape`, `Boundary`, `bool`) are defined on `base.xsd` types and so carry
over for free.

Note that several schemas would have to be generated in a *single*
`xsd-parser` pass (`Config::parser::schemas` takes a list) rather than one
pass each: generated separately, every format would get its own incompatible
copy of `base.xsd`'s shared types.
