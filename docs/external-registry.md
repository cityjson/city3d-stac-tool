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

## Ownership boundary

- This repo owns CLI behavior, config parsing, schema validation, tests, and releases.
- The registry repo owns dataset config instances, contributor guidance, and publication CI.
- URL crawlers or private discovery automation should stay outside the public registry unless
  they are intentionally part of the contributor workflow.
