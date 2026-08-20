//! Reader- and filesystem-dependent constructors for [`StacItemBuilder`].
//!
//! These free functions build an Item from a [`CityModelMetadataReader`] and
//! a file on disk. They live in this crate, not `city3d-stac-types`, because
//! the pure builder there must not depend on either the reader trait or the
//! filesystem (`file:checksum` aside — see
//! [`city3d_stac_types::checksum::file_checksum`]).

use crate::error::Result;
use crate::metadata::CRS;
use crate::reader::CityModelMetadataReader;
use crate::stac::StacItemBuilder;
use city3d_stac_types::checksum::file_checksum;
use city3d_stac_types::stac::types::{Item, Link};
use city3d_stac_types::stac::ItemMetadata;
use std::path::Path;

/// Read an existing Item JSON file from disk and extract its [`ItemMetadata`].
///
/// This is the filesystem-dependent counterpart of
/// [`ItemMetadata::from_item`], which is pure and lives in
/// `city3d-stac-types`. It belongs here, not there, for the same reason the
/// rest of this module does: the types crate performs no I/O (see
/// `city3d_stac_types::error`'s module doc), with `file_checksum` as the one
/// sanctioned exception because `file:checksum` is content-derived rather
/// than filesystem-derived.
pub fn item_metadata_from_file(path: &Path) -> Result<ItemMetadata> {
    let content = std::fs::read_to_string(path)?;
    let item: Item = serde_json::from_str(&content)?;
    Ok(ItemMetadata::from_item(&item))
}

/// Resolve CRS from reader, using the override as fallback when the reader's CRS is unknown.
pub(crate) fn resolve_crs(reader: &dyn CityModelMetadataReader, crs_override: Option<&CRS>) -> CRS {
    let crs = reader.crs().unwrap_or_default();
    if crs.is_known() {
        crs
    } else if let Some(override_crs) = crs_override {
        override_crs.clone()
    } else {
        crs
    }
}

/// Map encoding name to IANA/vendor media type
fn encoding_media_type(encoding: &str) -> &'static str {
    match encoding {
        "CityJSON" => "application/city+json",
        "CityJSONSeq" => "application/city+json-seq",
        "CityGML" => "application/gml+xml",
        "FlatCityBuf" => "application/vnd.flatcitybuf",
        _ => "application/octet-stream",
    }
}

/// Build an Item builder from a file path
pub fn item_from_file(
    file_path: &Path,
    reader: &dyn CityModelMetadataReader,
    base_url: Option<&str>,
    original_url: Option<&str>,
) -> Result<StacItemBuilder> {
    item_from_file_with_crs_override(file_path, reader, base_url, original_url, None)
}

/// Build an Item builder from a file path with an optional CRS override
pub fn item_from_file_with_crs_override(
    file_path: &Path,
    reader: &dyn CityModelMetadataReader,
    base_url: Option<&str>,
    original_url: Option<&str>,
    crs_override: Option<&CRS>,
) -> Result<StacItemBuilder> {
    let id = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    build_item_from_file(id, file_path, reader, base_url, original_url, crs_override)
}

/// Build an Item builder from a file path with format suffix and optional CRS override
pub fn item_from_file_with_format_suffix_and_crs(
    file_path: &Path,
    reader: &dyn CityModelMetadataReader,
    base_url: Option<&str>,
    original_url: Option<&str>,
    crs_override: Option<&CRS>,
) -> Result<StacItemBuilder> {
    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let suffix = match reader.encoding() {
        "CityJSON" => "_cj",
        "CityJSONSeq" => "_cjseq",
        "FlatCityBuf" => "_fcb",
        _ => "",
    };

    let id = format!("{stem}{suffix}");

    build_item_from_file(id, file_path, reader, base_url, original_url, crs_override)
}

/// Shared body of the `item_from_file*` entry points above. Only the item ID
/// construction differs between callers; everything else — bbox handling,
/// `resolve_crs`, `city3d`, asset, link — must stay identical across both
/// public entry points. This function exists precisely so that never has to
/// be verified by inspection again: a `proj:code` bug once shipped because
/// two copies of this logic disagreed (see
/// `test_proj_code_matches_bbox_crs_when_reader_crs_unknown` below).
fn build_item_from_file(
    id: String,
    file_path: &Path,
    reader: &dyn CityModelMetadataReader,
    base_url: Option<&str>,
    original_url: Option<&str>,
    crs_override: Option<&CRS>,
) -> Result<StacItemBuilder> {
    let mut builder = StacItemBuilder::new(id);

    // Set bbox (transformed to WGS84 for STAC compliance)
    if let Ok(bbox) = reader.bbox() {
        let crs = resolve_crs(reader, crs_override);
        let wgs84_bbox = bbox.to_wgs84(&crs)?;
        builder = builder.bbox(wgs84_bbox).geometry_from_bbox();
    }

    // Add CityJSON metadata
    let props = crate::adapter::properties_from_reader(reader)?;
    // `proj:code` and the bbox transform above deliberately agree on the
    // same resolved CRS: both fall back to `crs_override` when the
    // reader's own CRS is unknown. Do not split them again — a reader
    // with an unknown CRS plus a supplied override should describe its
    // own coordinates in `proj:code`, not report them as unprojected.
    let resolved_crs = resolve_crs(reader, crs_override);
    builder = builder
        .datetime_from_reference_date(reader.metadata().ok().flatten().as_ref())
        .city3d(props)?
        .crs(&resolved_crs);

    // Get file size and checksum for the asset (File Extension)
    let file_size = std::fs::metadata(file_path).ok().map(|m| m.len());
    let checksum = file_checksum(file_path);

    // Add data asset - detect ZIP files for proper media type
    let is_zip = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "zip")
        .unwrap_or(false);

    let media_type = if is_zip {
        "application/zip"
    } else {
        encoding_media_type(reader.encoding())
    };

    // Generate asset href based on base_url or original_url
    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data");

    let href = match base_url {
        Some(base) => {
            let normalized_base = if base.ends_with('/') {
                base.to_string()
            } else {
                format!("{base}/")
            };
            format!("{normalized_base}{file_name}")
        }
        None => match original_url {
            Some(url) => url.to_string(),
            None => file_name.to_string(),
        },
    };

    builder = builder.data_asset(href.clone(), media_type, file_size, checksum);

    // Add city-model relation link (per STAC 3D City Models Extension)
    builder =
        builder.link(Link::new(&href, "city-model").with_media_type(Some(media_type.to_string())));

    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::CityJSONReader;
    use serde_json::Value;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_cityjson() -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        let cityjson = r#"{
            "type": "CityJSON",
            "version": "2.0",
            "transform": {
                "scale": [0.01, 0.01, 0.01],
                "translate": [100000, 200000, 0]
            },
            "metadata": {
                "geographicalExtent": [1.0, 2.0, 0.0, 10.0, 20.0, 30.0],
                "referenceSystem": "https://www.opengis.net/def/crs/EPSG/0/7415"
            },
            "CityObjects": {
                "building1": {
                    "type": "Building",
                    "geometry": [{
                        "type": "Solid",
                        "lod": "2",
                        "boundaries": [[[[0,0,0]]]]
                    }],
                    "attributes": {
                        "yearOfConstruction": 2020
                    }
                }
            },
            "vertices": [[0,0,0]]
        }"#;

        writeln!(temp_file, "{}", cityjson).unwrap();
        temp_file.flush().unwrap();
        temp_file
    }

    #[test]
    fn test_item_builder_with_cityjson() {
        let temp_file = create_test_cityjson();
        let reader = CityJSONReader::new(temp_file.path()).unwrap();

        let props = crate::adapter::properties_from_reader(&reader).unwrap();
        let resolved_crs = resolve_crs(&reader, None);
        let item = StacItemBuilder::new("test-building")
            .datetime_from_reference_date(reader.metadata().unwrap().as_ref())
            .city3d(props)
            .unwrap()
            .crs(&resolved_crs)
            .build()
            .unwrap();

        assert_eq!(
            item.properties
                .additional_fields
                .get("city3d:version")
                .unwrap(),
            "2.0"
        );
        assert_eq!(
            item.properties
                .additional_fields
                .get("city3d:city_objects")
                .unwrap(),
            1
        );
        assert_eq!(
            item.properties.additional_fields.get("proj:code").unwrap(),
            "EPSG:7415"
        );
    }

    #[test]
    fn test_item_builder_from_file() {
        let temp_file = create_test_cityjson();
        let reader = CityJSONReader::new(temp_file.path()).unwrap();

        let builder = item_from_file(temp_file.path(), &reader, None, None).unwrap();
        let item = builder.build().unwrap();

        assert!(item.bbox.is_some());
        assert!(item.geometry.is_some());
        assert!(item.assets.contains_key("data"));
    }

    #[test]
    fn test_item_data_asset_has_file_size_and_checksum() {
        let temp_file = create_test_cityjson();
        let reader = CityJSONReader::new(temp_file.path()).unwrap();

        let item = item_from_file(temp_file.path(), &reader, None, None)
            .unwrap()
            .build()
            .unwrap();

        let data_asset = item.assets.get("data").unwrap();

        // file:size present and positive
        let size = data_asset
            .additional_fields
            .get("file:size")
            .and_then(|v| v.as_u64())
            .unwrap();
        assert!(size > 0);

        // file:checksum present as a SHA-256 multihash hex string
        let checksum = data_asset
            .additional_fields
            .get("file:checksum")
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(checksum.starts_with("1220"));
        assert_eq!(checksum.len(), 4 + 64);

        // File Extension declared
        assert!(item
            .extensions
            .iter()
            .any(|e| e.contains("stac-extensions.github.io/file/")));
    }

    #[test]
    fn test_proj_code_matches_bbox_crs_when_reader_crs_unknown() {
        // Regression test: `proj:code` and the bbox transform must resolve the
        // same CRS. `railway.city.json` has no `referenceSystem`, so the
        // reader's own CRS is unknown. With an override supplied, both the
        // bbox reprojection and `proj:code` should use it — the Item must not
        // under-describe coordinates it did in fact reproject.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("railway.city.json");
        let reader = CityJSONReader::new(&path).unwrap();
        assert!(
            !reader.crs().unwrap().is_known(),
            "fixture must have an unknown CRS for this test to be meaningful"
        );

        let override_crs = CRS::from_epsg(7415);
        let item =
            item_from_file_with_crs_override(&path, &reader, None, None, Some(&override_crs))
                .unwrap()
                .build()
                .unwrap();

        assert_eq!(
            item.properties.additional_fields.get("proj:code").unwrap(),
            &Value::String(override_crs.to_stac_proj_code().unwrap())
        );

        // Negative control: without an override, an unknown reader CRS
        // yields no `proj:code` at all.
        let item_no_override = item_from_file_with_crs_override(&path, &reader, None, None, None)
            .unwrap()
            .build()
            .unwrap();

        assert!(!item_no_override
            .properties
            .additional_fields
            .contains_key("proj:code"));
    }

    #[test]
    fn test_proj_code_matches_bbox_crs_when_reader_crs_unknown_format_suffix() {
        // Same regression as `test_proj_code_matches_bbox_crs_when_reader_crs_unknown`,
        // pinned separately for `item_from_file_with_format_suffix_and_crs` — the
        // entry point the collection CLI actually takes on filename collisions
        // (see `src/cli/mod.rs`). Both public entry points share
        // `build_item_from_file`, but that must stay explicit, not implied.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("railway.city.json");
        let reader = CityJSONReader::new(&path).unwrap();
        assert!(
            !reader.crs().unwrap().is_known(),
            "fixture must have an unknown CRS for this test to be meaningful"
        );

        let override_crs = CRS::from_epsg(7415);
        let item = item_from_file_with_format_suffix_and_crs(
            &path,
            &reader,
            None,
            None,
            Some(&override_crs),
        )
        .unwrap()
        .build()
        .unwrap();

        assert_eq!(
            item.properties.additional_fields.get("proj:code").unwrap(),
            &Value::String(override_crs.to_stac_proj_code().unwrap())
        );

        // Negative control: without an override, an unknown reader CRS
        // yields no `proj:code` at all.
        let item_no_override =
            item_from_file_with_format_suffix_and_crs(&path, &reader, None, None, None)
                .unwrap()
                .build()
                .unwrap();

        assert!(!item_no_override
            .properties
            .additional_fields
            .contains_key("proj:code"));
    }
}
