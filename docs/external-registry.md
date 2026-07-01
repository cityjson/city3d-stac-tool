# Using `cityjson-stac` From an External Registry Repo

The generator is designed to be consumed from a separate open-data registry repository.

## Recommended consumer layout

```text
opendata-registry/
├── catalog/
│   └── catalog-config.yaml
├── collections/
│   ├── netherlands-3d-bag.yaml
│   └── singapore.yaml
├── docs/
├── tools/
│   └── cityjson-stac/   # git submodule
└── .github/workflows/
```

## Recommended integration

Add this repository as a submodule:

```bash
git submodule add git@github.com:HideBa/city3d-stac-tool.git tools/cityjson-stac
```

Validate collection configs in CI:

```bash
cargo run --manifest-path tools/cityjson-stac/Cargo.toml -- \
  collection --config collections/example.yaml --dry-run
```

Generate and publish a full catalog:

```bash
cargo run --manifest-path tools/cityjson-stac/Cargo.toml -- \
  catalog --config catalog/catalog-config.yaml -o build/site
```

## Declared summaries for config-only collections

Some datasets only offer an interactive or area-based download (no directly
processable file URL). For these, set `inputs: []` and declare what would
otherwise be auto-detected from the files under `summaries:`. With no items
to derive a spatial extent from, `extent.spatial.bbox` must also be set
explicitly, or the run fails with
`No input files found and no spatial extent (bbox) in config`:

```yaml
inputs: []

extent:
  spatial:
    bbox: [24.7, 60.1, 25.3, 60.3]

summaries:
  city3d:lods:
  - 1
  - 2
  city3d:co_types:
  - Building
  - TINRelief
  city3d:textures:
  - true
```

For array-valued keys, declared values are **unioned** with anything
auto-detected from processed items — neither source overwrites the other.
This means the same config works unmodified if `inputs` is later populated:
the config-declared values simply supplement whatever the reader finds.
Non-array values (and keys with no auto-detected counterpart) are taken as-is
from the config.

## Ownership boundary

- This repo owns CLI behavior, config parsing, schema validation, tests, and releases.
- The registry repo owns dataset config instances, contributor guidance, and publication CI.
- URL crawlers or private discovery automation should stay outside the public registry unless
  they are intentionally part of the contributor workflow.
