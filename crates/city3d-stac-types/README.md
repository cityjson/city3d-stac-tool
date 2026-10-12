# city3d-stac-types

Types and serialiser for [STAC](https://stacspec.org/) Items describing 3D
city models, using the [`city3d` STAC extension](https://github.com/cityjson/stac-city3d).

This is the dependency-light half of
[city3d-stac-tool](https://github.com/cityjson/city3d-stac-tool). It contains
`StacItemBuilder`, `City3dProperties`, the metadata types (`BBox3D`, `CRS`,
`Transform`, `AttributeDefinition`), SHA-256 checksums for the File extension,
and `extensions::CITY3D_EXTENSION`, the pinned `city3d` schema URL.

It has no async runtime, HTTP or object-store client, and no dependency on the
upstream `stac` crate. A writer that only needs to emit a STAC Item (for
example `cityparquet-rs`) can depend on it without pulling in a reader stack
or CLI. Its runtime dependencies are `serde`, `serde_json`, `chrono`,
`indexmap`, `thiserror`, `sha2`, `hex`, and `proj4rs`.

```toml
[dependencies]
city3d-stac-types = "0.1"
```

To read CityJSON, CityJSONSeq, CityGML or FlatCityBuf files, or to build
Collections and Catalogs, use
[`city3d-stac-gen`](https://crates.io/crates/city3d-stac-gen) instead, which
also provides the `city3dstac` CLI.

## License

MIT
