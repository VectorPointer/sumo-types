# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-08-09

### Added

- `additional` feature: reads `.add.xml` (SUMO "additional" files) into
  `additional::Additional` via `additional::read_additional`, modelling the
  E1 (induction loop), E2 (lane area) and E3 (multi-entry / multi-exit)
  detector definitions. Independent of `net` and `routes`. Everything else
  `additionalType` allows is silently dropped — see the README.

### Fixed

- The claim that `additional_file.xsd` could not be generated at all was
  wrong about its cause, and about being unfixable. It was never a diamond
  over `types/base.xsd`: the file drags `fileOptionType` in through
  `types/metadata.xsd`, which pulls in the 13 `*ConfigurationType.xsd`
  schemas. `build.rs` now carries targeted per-file patches, and the schema
  generates.

## [0.1.0] - 2026-08-09

First release. Reads two SUMO file formats into hand-written domain types,
one Cargo feature per format:

- `net` (default) — `.net.xml`, `netconvert`'s output, via `read_network`:
  edges, lanes, junctions, connections, roundabouts, traffic light programs,
  and the network's `location`/projection.
- `routes` — `.rou.xml` traffic demand, via `routes::read_routes`: vehicle
  types, routes, and vehicles.

Both are generated at build time from Eclipse SUMO's own XSD schemas
(vendored under `xsd/`, redistributed under EPL-2.0 — see `NOTICE`), then
mapped to types that don't leak the schema layer. Physical quantities are
[`uom`](https://crates.io/crates/uom) types rather than bare numbers.

[Unreleased]: https://github.com/VectorPointer/sumo-types/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/VectorPointer/sumo-types/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/VectorPointer/sumo-types/releases/tag/v0.1.0
