# City3D STAC Tool Guidelines

This repository is the Rust CLI and library implementation for generating STAC from 3D city model datasets.

## Repo purpose

- Own the `city3dstac` CLI implementation
- Own config parsing, validation, readers, STAC builders, and tests
- Own release automation for the generator binary
- Remain reusable from external registry repositories

## Workspace layout

The workspace has two crates:

- **`crates/city3d-stac-types`** (`city3d-stac-types`) — the dependency-light
  vocabulary crate: `City3dProperties`, `StacItemBuilder`, the local STAC
  serde model (`stac::types`), metadata types (`BBox3D`, `CRS`,
  `AttributeDefinition`, …), checksums, and the `city3d` extension URL
  constant (`extensions::CITY3D_EXTENSION`, pinned to a released schema
  version). It is meant to be depended on by external writers (e.g.
  `cityparquet-rs`) that need to emit a STAC Item without pulling in a
  reader, a CLI, or an async runtime.

  **Rule: this crate must never gain a runtime dependency on `stac`,
  `tokio`, `object_store`, or `reqwest`.** Its exact runtime dependency
  budget is `serde`, `serde_json`, `chrono`, `indexmap`, `thiserror`,
  `sha2`, `hex`, `proj4rs` (see `crates/city3d-stac-types/Cargo.toml`).
  `jsonschema` is a dev-dependency only, used by
  `tests/schema_conformance_tests.rs` to validate emitted Items against
  the vendored `schemas/stac-city3d-v0.2.0.json` fixture — never move it
  to `[dependencies]`. Verify with:

  ```bash
  cargo tree -p city3d-stac-types --edges normal
  ```

- **`crates/city3d-stac-gen`** (lib `city3d_stac`, bin `city3dstac`) — the
  CLI and format readers (CityJSON, CityJSONSeq, CityGML, ZIP, FlatCityBuf),
  the `CityModelMetadataReader` trait, directory traversal, and the
  `StacCollectionBuilder` / `StacCatalogBuilder`. `collection.rs` and
  `catalog.rs` deliberately stay here (not in the types crate) because they
  depend on the upstream `stac` crate; the external consumer only needs to
  emit a single Item, not a Collection or Catalog, so that dependency isn't
  worth pushing down.

The golden-output tests (`crates/city3d-stac-gen/tests/golden_output_tests.rs`
+ `tests/golden/`) pin the CLI's byte-for-byte output and must stay
unchanged across refactors; `UPDATE_GOLDEN=1` is forbidden.

The vendored schema in `crates/city3d-stac-types/schemas/` is a drift
guard: if the published `stac-city3d` schema moves past the version pinned
by `CITY3D_EXTENSION`, or if `City3dProperties` ever emits a shape the
extension forbids, `schema_conformance_tests.rs` fails in this crate —
before it can surface as invalid output in a downstream consumer.

## Scope boundary

- Public dataset registry content does not belong here
- Collection instances, catalog membership, contributor docs for public datasets, and publication-site policy belong in the separate registry repo
- This tool repo should stay registry-agnostic except for examples and integration documentation

## Technical guidance

- Preserve the current separation between readers, metadata, config handling, and STAC generation
- Changes to config semantics must be documented and should consider compatibility with external consumer repos
- Prefer improving dry-run validation and stable CLI behavior over adding repo-specific assumptions
- Keep the CLI usable both from source (`cargo run`) and as an installed binary (`cargo install --git ... --bin city3dstac`)

## Verification baseline

Before committing tool changes, run:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Consumer integration

- The main consumer is a registry repo that vendors this repository as `tools/cityjson-stac`
- Keep `docs/external-registry.md` up to date when integration expectations change
