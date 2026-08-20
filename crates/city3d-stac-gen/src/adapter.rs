//! One-way bridge from a format reader to the shared `City3dProperties` DTO.
//!
//! This is the only place the reader trait meets the types crate. The trait
//! itself never crosses the crate boundary: a writer such as `cityparquet-rs`
//! builds a `City3dProperties` directly and never implements a reader.

use city3d_stac_types::stac::{City3dProperties, CityObjectsCount};

use crate::error::Result;
use crate::reader::CityModelMetadataReader;

/// Collect the `city3d:*` property set from a metadata reader.
///
/// Each accessor is treated as best-effort: a reader that cannot answer a
/// question leaves that field absent rather than failing the whole item. This
/// mirrors the `if let Ok(...)` behaviour of the builder method it replaces.
pub fn properties_from_reader(reader: &dyn CityModelMetadataReader) -> Result<City3dProperties> {
    Ok(City3dProperties {
        version: reader.version().ok(),
        lods: reader.lods().unwrap_or_default(),
        co_types: reader.city_object_types().unwrap_or_default(),
        city_objects: reader
            .city_object_count()
            .ok()
            .map(|c| CityObjectsCount::Integer(c as u64)),
        semantic_surfaces: reader.semantic_surfaces().ok(),
        textures: reader.textures().ok(),
        materials: reader.materials().ok(),
        attributes: reader.attributes().unwrap_or_default(),
    })
}
