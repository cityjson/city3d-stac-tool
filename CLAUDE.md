# City3D STAC Tool Guidelines

This repository is the Rust CLI and library implementation for generating STAC from 3D city model datasets.

## Repo purpose

- Own the `city3dstac` CLI implementation
- Own config parsing, validation, readers, STAC builders, and tests
- Own release automation for the generator binary
- Remain reusable from external registry repositories

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
