# Technical Design Document

This document provides detailed technical architecture and implementation guidelines for the City3D STAC tool (`city3dstac`).

## Table of Contents

0. [Workspace Layout](#workspace-layout)
1. [Architecture Details](#architecture-details)
2. [Design Philosophy](#design-philosophy)
3. [Development Standards](#development-standards)
4. [Test Data](#test-data)
5. [Module Design](#module-design)
6. [Data Flow](#data-flow)
7. [CLI Specification](#cli-specification)
8. [Error Handling](#error-handling)
9. [Testing Strategy](#testing-strategy)

---

## Workspace Layout

The repository is a two-crate Cargo workspace. Both crates are published to
crates.io and released in lockstep (same semver bump level).

```
┌──────────────────────────────────────────────────────────────┐
│ city3d-stac-gen  (crates/city3d-stac-gen)                    │
│   lib `city3d_stac`, bin `city3dstac`                        │
│   CLI, config, readers (CityJSON, CityJSONSeq, CityGML,      │
│   FlatCityBuf, ZIP, GZIP), remote access, traversal,         │
│   StacCollectionBuilder / StacCatalogBuilder, GeoParquet     │
│   deps: stac, tokio, object_store, reqwest, …                │
└──────────────────────────────┬───────────────────────────────┘
                               │ depends on
                               ▼
┌──────────────────────────────────────────────────────────────┐
│ city3d-stac-types  (crates/city3d-stac-types)                │
│   StacItemBuilder, City3dProperties, local STAC serde model, │
│   metadata (BBox3D, CRS, Transform, AttributeDefinition),    │
│   checksums, extensions::CITY3D_EXTENSION                    │
│   deps: serde, serde_json, chrono, indexmap, thiserror,      │
│         sha2, hex, proj4rs — nothing else at runtime         │
└──────────────────────────────────────────────────────────────┘
```

`city3d-stac-types` exists so that external writers (e.g. `cityparquet-rs`)
can emit a STAC Item without inheriting the CLI's reader stack or an async
runtime. It must never gain a runtime dependency on `stac`, `tokio`,
`object_store`, or `reqwest`. `city3d_stac::metadata` re-exports
`city3d_stac_types::metadata`, so both paths resolve to the same types.

The `CityModelMetadataReader` trait lives in `city3d-stac-gen`; the bridge
from a reader to `City3dProperties` is `city3d_stac::adapter::properties_from_reader`.

---

## Architecture Details

### Design Principles

1. **Trait-Based Extensibility**: New format readers implement `CityModelMetadataReader`
2. **Separation of Concerns**: Clear boundaries between reading, metadata extraction, and STAC generation
3. **Type Safety**: Leverage Rust's type system for correctness
4. **Performance**: Lazy loading and optional parallel processing

### Core Trait Definition

```rust
/// Common interface for all format readers
/// (crates/city3d-stac-gen/src/reader/mod.rs)
pub trait CityModelMetadataReader: Send + Sync {
    /// Extract 3D bounding box [xmin, ymin, zmin, xmax, ymax, zmax]
    fn bbox(&self) -> Result<BBox3D>;

    /// Get coordinate reference system
    fn crs(&self) -> Result<CRS>;

    /// Get available levels of detail
    fn lods(&self) -> Result<Vec<String>>;

    /// Get city object types present
    fn city_object_types(&self) -> Result<Vec<String>>;

    /// Count total city objects
    fn city_object_count(&self) -> Result<usize>;

    /// Extract attribute schema definitions
    fn attributes(&self) -> Result<Vec<AttributeDefinition>>;

    /// Get encoding format name
    fn encoding(&self) -> &'static str;

    /// Get CityJSON version
    fn version(&self) -> Result<String>;

    /// Get file path
    fn file_path(&self) -> &Path;

    /// Get coordinate transform parameters
    fn transform(&self) -> Result<Option<Transform>>;

    /// Extract additional metadata
    fn metadata(&self) -> Result<Option<serde_json::Value>>;

    /// Get the CityJSON extensions used
    fn extensions(&self) -> Result<Vec<String>>;

    /// Whether semantic surfaces / textures / materials are present
    fn semantic_surfaces(&self) -> Result<bool>;
    fn textures(&self) -> Result<bool>;
    fn materials(&self) -> Result<bool>;

    /// Whether the format can be processed as a stream (default: false)
    fn streamable(&self) -> bool { false }
}
```

### Reader Factory Pattern

`get_reader(path)` in `crates/city3d-stac-gen/src/reader/mod.rs` picks a
reader by file extension (`get_reader_from_source` does the same for remote
inputs):

| Extension | Reader |
| --------- | ------ |
| `.json` | `CityJSONReader` |
| `.jsonl` | `CityJSONSeqReader` |
| `.gml`, `.xml` | `CityGMLReader` (2.0 / 3.0), after checking the content is CityGML |
| `.fcb` | `FlatCityBufReader` |
| `.zip` | `ZipReader` (aggregates the supported files inside) |
| `.gz` | `GzipReader` (wraps the reader for the inner file) |

Any other extension is rejected with an error.

---

## Design Philosophy

### Interface Programming

The codebase follows a **trait-oriented design** where all format readers implement the `CityModelMetadataReader` trait. This provides:

1. **Polymorphism**: Callers work with `dyn CityModelMetadataReader` without knowing the concrete implementation
2. **Extensibility**: New formats can be added by implementing the trait without changing existing code
3. **Testability**: Mock implementations can be easily created for testing

### Factory Pattern

The `get_reader()` function serves as a factory that:
- Examines file extensions to determine format
- Returns a boxed trait object (`Box<dyn CityModelMetadataReader>`)
- Encapsulates reader creation logic
- Makes the codebase open for extension (new readers) but closed for modification

### Streaming-First Approach

Different formats have different characteristics:
- **CityJSON (`.json`)**: Single-file format, typically small enough to load entirely
- **CityJSONSeq (`.jsonl`)**: Designed for streaming, processes line-by-line
- **FlatCityBuf (`.fcb`)**: Binary format with header metadata that can be read without full file load

The reader implementations respect these characteristics:
- Use lazy loading with `RwLock` for interior mutability
- Stream `.jsonl` files line-by-line when possible
- Cache extracted metadata to avoid redundant computation

### Data Source Abstraction

The `InputSource` enum abstracts between local and remote data:

```rust
pub enum InputSource {
    Local(PathBuf),
    Remote(String),  // http://, https://, s3://, az://, gs://
}
```

This design allows:
- Future support for HTTP/HTTPS and object storage (S3, Azure, GCS)
- Transparent switching between local and remote data
- Consistent API regardless of data location

### Module Dependency Direction

```
┌─────────────────────────────────────────────────────────┐
│                    STAC Module                          │
│          (consumes metadata from readers)               │
└────────────────────┬────────────────────────────────────┘
                     │ depends on
                     ▼
┌─────────────────────────────────────────────────────────┐
│                   Reader Module                         │
│   (implements CityModelMetadataReader trait)            │
└────────────────────┬────────────────────────────────────┘
                     │ depends on
                     ▼
┌─────────────────────────────────────────────────────────┐
│                 Metadata Module                         │
│       (BBox3D, CRS, Transform, AttributeDefinition)     │
└─────────────────────────────────────────────────────────┘
```

**Key principle**: Readers depend on metadata types, not vice versa. The metadata module has no knowledge of readers or CityJSON formats. The Metadata module (and the STAC Item builder) live in `city3d-stac-types`; the Reader module and the Collection/Catalog builders live in `city3d-stac-gen`.

---

## Development Standards

### Test-Driven Development (TDD)

When adding new features or modifying existing ones:
1. Write tests first to define expected behavior
2. Run tests to see them fail (red)
3. Implement the minimum code to make tests pass (green)
4. Refactor while keeping tests green

### Code Quality Standards

**Pre-commit checklist** (enforced by git hooks):
```bash
# Format code
cargo fmt

# Check for lints and warnings
cargo clippy --all-targets --all-features -- -D warnings

# Run unit tests (the git hook runs this subset)
cargo test --lib

# Before committing, run the full suite (both crates)
cargo test
```

**Rules**:
- **NEVER** use `#[allow(dead_code)]` or `#[allow(unused)]` as workarounds
- If code is truly not needed, remove it
- If clippy warns about dead code during development, consider marking tests as `#[cfg(test)]`
- If clippy warns about unused code that will be used later, consider using `#[expect(dead_code)]` with a comment explaining why

### Testing Guidelines

1. **Return mocked data in tests, not real data**
   - Tests should be deterministic and fast
   - Create minimal test fixtures inline in the test function
   - Avoid reading external files in unit tests (use integration tests for file I/O)

2. **Use descriptive test names**
   ```rust
   #[test]
   fn test_bbox_merge_returns_union_of_both_boxes() { ... }
   ```

3. **Test both success and failure paths**
   - Verify error handling with invalid inputs
   - Check that error messages are helpful

4. **Organize tests by module**
   - Unit tests in inline `#[cfg(test)]` modules
   - Integration tests in `crates/<crate>/tests/`

---

## Test Data

### Test Data Location

Test fixtures for the CLI and readers are located in `crates/city3d-stac-gen/tests/data/`:

| File | Format | Description | Use Case |
|------|--------|-------------|----------|
| `delft.city.json` | CityJSON | Small file with metadata but no geometry | Basic reader tests |
| `delft.city.jsonl` | CityJSONSeq | 159 lines (1 header + 158 features) | Streaming tests |
| `railway.city.json` | CityJSON | Complex file with geometry and attributes | Full metadata extraction |
| `railway.city.jsonl` | CityJSONSeq | 39 lines (1 header + 38 features) | Smaller streaming dataset |
| `all.fcb` | FlatCityBuf | Binary format reference | FCB reader tests |
| `3dbag_citygml2.gml` | CityGML 2.0 | 3DBAG tile | CityGML 2.0 reader tests |
| `3dbag_citygml3.gml` | CityGML 3.0 | 3DBAG tile | CityGML 3.0 reader tests |
| `prefixed_citygml2.gml` | CityGML 2.0 | Non-default namespace prefixes | Namespace handling |
| `datetime_fixture/` | CityJSON | File with a `referenceDate` | Item datetime tests |

`crates/city3d-stac-gen/tests/golden/` holds the golden outputs pinned by
`golden_output_tests.rs`. The types crate's own tests (builder, serde, schema
conformance) live in `crates/city3d-stac-types/tests/` and build their inputs
inline; the vendored extension schema is in `crates/city3d-stac-types/schemas/`.

### Using Test Data in Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_data_path(file: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(file)
    }

    #[test]
    fn test_delft_cityjson() {
        let path = test_data_path("delft.city.json");
        let reader = CityJSONReader::new(&path).unwrap();
        // ... test assertions
    }
}
```

### Adding New Test Files

When adding new test data:
1. Place the file in `crates/city3d-stac-gen/tests/data/`
2. Document its purpose in the table above
3. Add a corresponding test that uses it
4. Keep test files minimal but representative of real-world data

---

## Module Design

### Metadata Structures

#### BBox3D

```rust
/// 3D Bounding box [xmin, ymin, zmin, xmax, ymax, zmax]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BBox3D {
    pub xmin: f64,
    pub ymin: f64,
    pub zmin: f64,
    pub xmax: f64,
    pub ymax: f64,
    pub zmax: f64,
}

impl BBox3D {
    /// Merge two bounding boxes (union)
    pub fn merge(&self, other: &BBox3D) -> BBox3D;

    /// Convert to STAC bbox array format
    pub fn to_array(&self) -> [f64; 6];

    /// Get 2D footprint (for STAC geometry)
    pub fn footprint_2d(&self) -> [f64; 4];
}
```

#### CRS (Coordinate Reference System)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CRS {
    /// EPSG code (e.g., 7415)
    pub epsg: Option<u32>,
    /// WKT2 representation
    pub wkt2: Option<String>,
    /// CityJSON authority/identifier
    pub authority: Option<String>,
    pub identifier: Option<String>,
}
```

#### Transform

```rust
/// Coordinate transform for vertex compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub scale: [f64; 3],
    pub translate: [f64; 3],
}
```

#### AttributeDefinition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub attr_type: AttributeType,
    pub description: Option<String>,
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttributeType {
    String, Number, Boolean, Date, Array, Object,
}
```

### STAC Builders

#### StacItemBuilder

Lives in `city3d-stac-types` (`city3d_stac_types::stac::StacItemBuilder`,
re-exported as `city3d_stac::stac::StacItemBuilder`). Abridged:

```rust
impl StacItemBuilder {
    pub fn new(id: impl Into<String>) -> Self;
    pub fn bbox(self, bbox: BBox3D) -> Self;
    pub fn geometry(self, geometry: Value) -> Self;
    pub fn geometry_from_bbox(self) -> Self;
    pub fn datetime(self, dt: Option<String>) -> Self;
    pub fn title(self, title: impl Into<String>) -> Self;
    pub fn description(self, description: impl Into<String>) -> Self;
    pub fn property(self, key: impl Into<String>, value: Value) -> Self;
    pub fn city3d(self, props: City3dProperties) -> Result<Self>;
    pub fn crs(self, crs: &CRS) -> Self;
    pub fn data_asset(self, /* href, media type, … */) -> Self;
    pub fn asset(self, key: impl Into<String>, asset: Asset) -> Self;
    pub fn link(self, link: Link) -> Self;
    pub fn collection_id(self, id: impl Into<String>) -> Self;
    pub fn build(self) -> Result<Item>;
}
```

Reader metadata reaches the builder as `City3dProperties`, produced by
`city3d_stac::adapter::properties_from_reader(&*reader)`.
`city3d_stac::stac::item_from_file` wires a reader into a builder the way the
CLI's `item` command does.

#### StacCollectionBuilder

Lives in `city3d-stac-gen` (`city3d_stac::stac::StacCollectionBuilder`)
because it builds an upstream `stac::Collection`. Abridged:

```rust
impl StacCollectionBuilder {
    pub fn new(id: impl Into<String>) -> Self;
    pub fn title(self, title: impl Into<String>) -> Self;
    pub fn description(self, description: impl Into<String>) -> Self;
    pub fn license(self, license: impl Into<String>) -> Self;
    pub fn spatial_extent(self, bbox: BBox3D) -> Self;
    pub fn temporal_extent(self, /* start, end */) -> Self;
    pub fn summary(self, key: impl Into<String>, value: Value) -> Self;
    pub fn item_link(self, href: impl ToString, title: Option<String>) -> Self;
    pub fn aggregate_cityjson_metadata(self, readers: &[Box<dyn CityModelMetadataReader>]) -> Result<Self>;
    pub fn aggregate_from_items(self, items: &[StacItem]) -> Result<Self>;
    pub fn build(self) -> Result<stac::Collection>;
}
```

`StacCatalogBuilder` (`stac/catalog.rs`) builds the catalog for the
`catalog` command.

---

## Data Flow

### Single File Processing

```
User Command: city3dstac item building.json
     │
     ▼
Parse CLI Arguments
     │
     ├── file: "building.json"
     ├── output: None (use default)
     │
     ▼
Reader Factory
     │
     ├── Check extension: ".json"
     ├── Create CityJSONReader
     │
     ▼
Extract Metadata
     │
     ├── Load JSON (lazy)
     ├── Extract bbox from vertices or metadata
     ├── Extract CRS from referenceSystem
     ├── Scan geometry for LODs
     ├── Collect city object types
     ├── Count objects
     ├── Build attribute schema
     │
     ▼
Build STAC Item
     │
     ├── StacItemBuilder::new()
     ├── .bbox(metadata.bbox)
     ├── .city3d(properties_from_reader(&reader))
     ├── .data_asset(path, media_type)
     ├── .build()
     │
     ▼
Write Output → building_item.json
```

### Directory Processing

```
User Command: city3dstac collection ./buildings/
     │
     ▼
Directory Traversal (walkdir)
     │
     ├── Filter by extensions: [json, jsonl, fcb]
     ├── Respect max-depth if set
     │
     ▼
Process Each File
     │
     ├── Create reader
     ├── Extract metadata
     ├── Build STAC Item
     ├── Store for aggregation
     │
     ▼
Aggregate Metadata
     │
     ├── Merge all bboxes → collection bbox
     ├── Union all LODs
     ├── Union all city object types
     ├── Union encodings
     ├── Calculate count statistics
     │
     ▼
Generate Collection + Items
     │
     ├── Write collection.json
     ├── Write items/*.json
     │
     ▼
Output Structure:
    stac_output/
    ├── collection.json
    └── items/
        ├── building_001_item.json
        └── ...
```

---

## CLI Specification

The binary is `city3dstac` with the commands `item`, `collection`,
`catalog`, and `update-collection` (alias `aggregate`). The README's
[CLI Reference](README.md#cli-reference) and `city3dstac <command> --help`
are the authoritative option lists; they are not duplicated here.

### Exit Codes

| Code | Meaning |
| ---- | ------- |
| 0    | Success |
| 1    | Any error (unsupported format, I/O, metadata extraction, write failure) |

With `--dry-run`, the exit code reports the validation result:
`0` valid, `1` config error, `2` missing input paths, `3` remote URL
inaccessible.

---

## Error Handling

### Error Types

Each crate has its own `thiserror` enum:

- `city3d_stac_types::City3dError` — JSON, metadata, missing-field, STAC,
  GeoJSON, and reprojection errors raised while building an Item.
- `city3d_stac::error::CityJsonStacError` — everything the CLI and readers
  can hit: unsupported format, I/O, JSON, metadata extraction, invalid
  CityJSON, missing field, STAC generation, invalid URL, GeoJSON, no
  supported files found, object storage, path, and storage/network errors.
  `City3dError` converts into it transparently.

### Error Handling Strategy

1. **Graceful degradation**: Continue processing other files if one fails
2. **Informative messages**: Include file path and specific error details
3. **Skip-errors mode**: Default behavior for directory processing

---

## Testing Strategy

### Test Pyramid

```
       /\
      /E2E\         End-to-end CLI tests
     /------\
    /  INT   \      Module integration tests
   /----------\
  /    UNIT    \    Function-level tests
 /--------------\
```

### Test Categories

1. **Unit Tests**: Individual functions in each module
2. **Integration Tests**: Reader → STAC builder flow
3. **E2E Tests**: Full CLI command execution

### Test Fixtures

See [Test Data](#test-data). Unit tests build minimal inputs inline;
integration tests read the files in `crates/city3d-stac-gen/tests/data/`.

### Coverage Target

- Unit tests: 80%+ coverage
- Integration tests: Critical paths
- E2E tests: All CLI commands

---

## FlatCityBuf Integration

`.fcb` files are read by `FlatCityBufReader`
(`crates/city3d-stac-gen/src/reader/fcb.rs`) using the
[`fcb_core`](https://github.com/cityjson/flatcitybuf) crate's `FcbReader`.
Bounding box, CRS, transform and attribute columns come from the FCB header
(cached on first access); city object counts and types need a pass over the
features, which is loaded lazily and cached separately.

---

## Performance Considerations

### Lazy Loading

Readers should defer file parsing until metadata is actually requested:

```rust
pub struct CityJSONReader {
    file_path: PathBuf,
    data: Option<Value>,  // Loaded on first access
}

impl CityJSONReader {
    fn ensure_loaded(&mut self) -> Result<&Value> {
        if self.data.is_none() {
            let file = File::open(&self.file_path)?;
            self.data = Some(serde_json::from_reader(BufReader::new(file))?);
        }
        Ok(self.data.as_ref().unwrap())
    }
}
```

### Parallel Processing

Optional parallel file processing for large directories:

```rust
#[cfg(feature = "parallel")]
use rayon::prelude::*;

// With --parallel flag:
files.par_iter()
    .map(|f| get_reader(f).and_then(|r| process(r)))
    .collect::<Vec<_>>()
```

### Memory Management

- Stream large `.jsonl` files line-by-line
- Use FlatCityBuf header for metadata without loading full file
- Limit concurrent readers in parallel mode
