//! STAC Collection builder

use crate::error::{CityJsonStacError, Result};
use crate::metadata::BBox3D;
use crate::reader::CityModelMetadataReader;
use chrono::{DateTime, Utc};
use city3d_stac_types::extensions::{
    CITY3D_EXTENSION, FILE_EXTENSION, ITEM_ASSETS_EXTENSION, PROJECTION_EXTENSION,
};
use indexmap::IndexMap;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

/// Build the `file:size` collection summary as a Range Object with an extra `mean`,
/// per the STAC File / Statistics extension convention. `mean` is the integer-rounded
/// average of the observed sizes. Returns `None` when no sizes were observed.
fn file_size_summary(
    size_min: Option<u64>,
    size_max: Option<u64>,
    size_sum: u128,
    size_count: u64,
) -> Option<Value> {
    let (min, max) = (size_min?, size_max?);
    if size_count == 0 {
        return None;
    }
    let mean = ((size_sum + (size_count as u128) / 2) / (size_count as u128)) as u64;
    Some(serde_json::json!({
        "minimum": min,
        "maximum": max,
        "mean": mean,
    }))
}

/// Builder for STAC Collections
pub struct StacCollectionBuilder {
    id: String,
    title: Option<String>,
    description: Option<String>,
    license: String,
    keywords: Option<Vec<String>>,
    providers: Option<Vec<stac::Provider>>,
    spatial_bboxes: Vec<stac::Bbox>,
    temporal_start: Option<DateTime<Utc>>,
    temporal_end: Option<DateTime<Utc>>,
    summaries: HashMap<String, Value>,
    links: Vec<stac::Link>,
    assets: IndexMap<String, stac::Asset>,
    item_assets: IndexMap<String, stac::ItemAsset>,
    /// Whether this collection has processed items (affects extension declarations)
    has_items: bool,
}

impl StacCollectionBuilder {
    /// Create a new STAC Collection builder
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            description: None,
            license: "proprietary".to_string(),
            keywords: None,
            providers: None,
            spatial_bboxes: Vec::new(),
            temporal_start: None,
            temporal_end: None,
            summaries: HashMap::new(),
            links: Vec::new(),
            assets: IndexMap::new(),
            item_assets: IndexMap::new(),
            has_items: false,
        }
    }

    /// Set title
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set description
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set license
    pub fn license(mut self, license: impl Into<String>) -> Self {
        self.license = license.into();
        self
    }

    /// Set keywords
    pub fn keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = Some(keywords);
        self
    }

    /// Add a provider
    pub fn provider(mut self, provider: stac::Provider) -> Self {
        self.providers.get_or_insert_with(Vec::new).push(provider);
        self
    }

    /// Set the collection's spatial extent from a bounding box, replacing any
    /// previously set value. (Symmetric with `temporal_extent`; the prior
    /// push-based behaviour caused later callers — including configs — to
    /// supplement rather than override an earlier aggregate-derived bbox.)
    pub fn spatial_extent(mut self, bbox: BBox3D) -> Self {
        let arr = bbox.to_array();
        let stac_bbox =
            stac::Bbox::ThreeDimensional([arr[0], arr[1], arr[2], arr[3], arr[4], arr[5]]);
        self.spatial_bboxes = vec![stac_bbox];
        self
    }

    /// Set temporal extent
    pub fn temporal_extent(
        mut self,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Self {
        self.temporal_start = start;
        self.temporal_end = end;
        self
    }

    /// Add a summary property
    pub fn summary(mut self, key: impl Into<String>, value: Value) -> Self {
        self.summaries.insert(key.into(), value);
        self
    }

    /// Add a summary property, taking the union with any existing array value
    /// for the same key instead of overwriting it.
    ///
    /// Used to merge config-declared summaries (e.g. `city3d:lods` sourced from
    /// a curated dataset index, for collections where inputs can't be read
    /// directly) with values auto-detected from processed items, so neither
    /// source clobbers the other. Elements are deduplicated by their string/
    /// number/bool representation; non-array values, or a key with no prior
    /// array value, fall back to a plain overwrite.
    pub fn summary_union(mut self, key: impl Into<String>, value: Value) -> Self {
        let key = key.into();

        fn dedup_key(v: &Value) -> Option<String> {
            match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(format!("bool:{b}")),
                _ => None,
            }
        }

        let merged = match (self.summaries.get(&key), &value) {
            (Some(Value::Array(existing)), Value::Array(incoming)) => {
                let mut unioned: std::collections::BTreeMap<String, Value> =
                    std::collections::BTreeMap::new();
                for v in existing.iter().chain(incoming.iter()) {
                    if let Some(k) = dedup_key(v) {
                        unioned.entry(k).or_insert_with(|| v.clone());
                    }
                }
                Value::Array(unioned.into_values().collect())
            }
            _ => value,
        };

        self.summaries.insert(key, merged);
        self
    }

    /// Add a link
    pub fn link(mut self, link: stac::Link) -> Self {
        self.links.push(link);
        self
    }

    /// Add a self link
    pub fn self_link(mut self, href: impl ToString) -> Self {
        self.links.push(stac::Link::self_(href));
        self
    }

    /// Add a parent link (points to parent catalog or collection)
    pub fn parent_link(mut self, href: impl ToString) -> Self {
        self.links.push(stac::Link::parent(href));
        self
    }

    /// Add a root link (points to root catalog)
    pub fn root_link(mut self, href: impl ToString) -> Self {
        self.links.push(stac::Link::root(href));
        self
    }

    /// Add an item link
    pub fn item_link(mut self, href: impl ToString, title: Option<String>) -> Self {
        let mut link = stac::Link::item(href);
        link.title = title;
        self.links.push(link);
        self
    }

    /// Add an asset
    pub fn asset(mut self, key: impl Into<String>, asset: stac::Asset) -> Self {
        self.assets.insert(key.into(), asset);
        self
    }

    /// Aggregate CityJSON metadata from multiple readers
    pub fn aggregate_cityjson_metadata(
        mut self,
        readers: &[Box<dyn CityModelMetadataReader>],
    ) -> Result<Self> {
        if !readers.is_empty() {
            self.has_items = true;
        }
        // Collect all versions
        let versions: HashSet<String> = readers.iter().filter_map(|r| r.version().ok()).collect();
        if !versions.is_empty() {
            let version_vec: Vec<String> = versions.into_iter().collect();
            self.summaries.insert(
                "city3d:version".to_string(),
                serde_json::to_value(version_vec)?,
            );
        }

        // Aggregate LODs as strings to avoid floating-point precision issues
        let all_lods: HashSet<String> = readers
            .iter()
            .filter_map(|r| r.lods().ok())
            .flatten()
            .collect();

        if !all_lods.is_empty() {
            let mut lods: Vec<String> = all_lods.into_iter().collect();
            lods.sort();
            self.summaries
                .insert("city3d:lods".to_string(), serde_json::to_value(lods)?);
        }

        // Aggregate city object types
        let all_types: HashSet<String> = readers
            .iter()
            .filter_map(|r| r.city_object_types().ok())
            .flatten()
            .collect();

        if !all_types.is_empty() {
            let mut types: Vec<String> = all_types.into_iter().collect();
            types.sort();
            self.summaries
                .insert("city3d:co_types".to_string(), serde_json::to_value(types)?);
        }

        // City object count statistics
        let counts: Vec<usize> = readers
            .iter()
            .filter_map(|r| r.city_object_count().ok())
            .collect();

        if !counts.is_empty() {
            let min = *counts.iter().min().unwrap();
            let max = *counts.iter().max().unwrap();
            let total: usize = counts.iter().sum();

            let stats = serde_json::json!({
                "min": min,
                "max": max,
                "total": total
            });

            self.summaries
                .insert("city3d:city_objects".to_string(), stats);
        }

        // Aggregate boolean fields as arrays of unique observed values
        let semantic_values: HashSet<bool> = readers
            .iter()
            .filter_map(|r| r.semantic_surfaces().ok())
            .collect();
        if !semantic_values.is_empty() {
            let mut vals: Vec<bool> = semantic_values.into_iter().collect();
            vals.sort();
            self.summaries.insert(
                "city3d:semantic_surfaces".to_string(),
                serde_json::to_value(vals)?,
            );
        }

        let texture_values: HashSet<bool> =
            readers.iter().filter_map(|r| r.textures().ok()).collect();
        if !texture_values.is_empty() {
            let mut vals: Vec<bool> = texture_values.into_iter().collect();
            vals.sort();
            self.summaries
                .insert("city3d:textures".to_string(), serde_json::to_value(vals)?);
        }

        let material_values: HashSet<bool> =
            readers.iter().filter_map(|r| r.materials().ok()).collect();
        if !material_values.is_empty() {
            let mut vals: Vec<bool> = material_values.into_iter().collect();
            vals.sort();
            self.summaries
                .insert("city3d:materials".to_string(), serde_json::to_value(vals)?);
        }

        // Aggregate proj:code
        let unique_proj_codes: HashSet<String> = readers
            .iter()
            .filter_map(|r| r.crs().ok())
            .filter_map(|crs| crs.to_stac_proj_code())
            .collect();

        if !unique_proj_codes.is_empty() {
            let mut codes: Vec<String> = unique_proj_codes.into_iter().collect();
            codes.sort();
            self.summaries
                .insert("proj:code".to_string(), serde_json::to_value(codes)?);
        }

        // Merge all bounding boxes for spatial extent (transformed to WGS84)
        let bboxes: Vec<BBox3D> = readers
            .iter()
            .filter_map(|r| {
                let bbox = r.bbox().ok()?;
                let crs = r.crs().unwrap_or_default();
                bbox.to_wgs84(&crs).ok()
            })
            .collect();

        if !bboxes.is_empty() {
            let mut merged = bboxes[0].clone();
            for bbox in &bboxes[1..] {
                merged = merged.merge(bbox);
            }
            self = self.spatial_extent(merged);
        }

        Ok(self)
    }

    /// Aggregate 3D City Models metadata from pre-aggregated summaries (streaming-friendly).
    ///
    /// Accepts `AggregatedSummaries` which are computed incrementally during item
    /// processing, avoiding the need to hold all ItemMetadata in memory at once.
    pub fn aggregate_from_summaries(
        mut self,
        summaries: &crate::stac::AggregatedSummaries,
    ) -> Result<Self> {
        self.has_items = true;

        if !summaries.versions.is_empty() {
            let version_vec: Vec<String> = summaries.versions.iter().cloned().collect();
            self.summaries.insert(
                "city3d:version".to_string(),
                serde_json::to_value(version_vec)?,
            );
        }

        if !summaries.lods.is_empty() {
            let mut lods: Vec<String> = summaries.lods.iter().cloned().collect();
            lods.sort();
            self.summaries
                .insert("city3d:lods".to_string(), serde_json::to_value(lods)?);
        }

        if !summaries.co_types.is_empty() {
            let mut types: Vec<String> = summaries.co_types.iter().cloned().collect();
            types.sort();
            self.summaries
                .insert("city3d:co_types".to_string(), serde_json::to_value(types)?);
        }

        if let (Some(min), Some(max)) = (summaries.count_min, summaries.count_max) {
            let stats = serde_json::json!({
                "min": min,
                "max": max,
                "total": summaries.count_total
            });
            self.summaries
                .insert("city3d:city_objects".to_string(), stats);
        }

        if !summaries.semantic_surfaces.is_empty() {
            let mut vals: Vec<bool> = summaries.semantic_surfaces.iter().copied().collect();
            vals.sort();
            self.summaries.insert(
                "city3d:semantic_surfaces".to_string(),
                serde_json::to_value(vals)?,
            );
        }

        if !summaries.textures.is_empty() {
            let mut vals: Vec<bool> = summaries.textures.iter().copied().collect();
            vals.sort();
            self.summaries
                .insert("city3d:textures".to_string(), serde_json::to_value(vals)?);
        }

        if !summaries.materials.is_empty() {
            let mut vals: Vec<bool> = summaries.materials.iter().copied().collect();
            vals.sort();
            self.summaries
                .insert("city3d:materials".to_string(), serde_json::to_value(vals)?);
        }

        if !summaries.proj_codes.is_empty() {
            let mut codes: Vec<String> = summaries.proj_codes.iter().cloned().collect();
            codes.sort();
            self.summaries
                .insert("proj:code".to_string(), serde_json::to_value(codes)?);
        }

        if let Some(file_size) = file_size_summary(
            summaries.size_min,
            summaries.size_max,
            summaries.size_sum,
            summaries.size_count,
        ) {
            self.summaries.insert("file:size".to_string(), file_size);
        }

        if let Some(merged) = &summaries.merged_bbox {
            self = self.spatial_extent(merged.clone());
        }

        Ok(self)
    }

    /// Aggregate metadata from pre-parsed STAC items
    pub fn aggregate_from_items(mut self, items: &[crate::stac::StacItem]) -> Result<Self> {
        if !items.is_empty() {
            self.has_items = true;
        }
        // Helper to extract string from item properties
        fn get_string(item: &crate::stac::StacItem, key: &str) -> Option<String> {
            item.properties
                .additional_fields
                .get(key)
                .and_then(|v| v.as_str())
                .map(String::from)
        }

        // Helper to extract string array from item properties
        fn get_string_array(item: &crate::stac::StacItem, key: &str) -> Vec<String> {
            item.properties
                .additional_fields
                .get(key)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        }

        fn get_int(item: &crate::stac::StacItem, key: &str) -> Option<i64> {
            item.properties
                .additional_fields
                .get(key)
                .and_then(|v| v.as_i64())
        }

        // Collect all versions
        let versions: HashSet<String> = items
            .iter()
            .filter_map(|item| get_string(item, "city3d:version"))
            .collect();
        if !versions.is_empty() {
            let version_vec: Vec<String> = versions.into_iter().collect();
            self.summaries.insert(
                "city3d:version".to_string(),
                serde_json::to_value(version_vec)?,
            );
        }

        // Aggregate LODs as strings to avoid floating-point precision issues
        let all_lods: HashSet<String> = items
            .iter()
            .flat_map(|item| get_string_array(item, "city3d:lods"))
            .collect();

        if !all_lods.is_empty() {
            let mut lods: Vec<String> = all_lods.into_iter().collect();
            lods.sort();
            self.summaries
                .insert("city3d:lods".to_string(), serde_json::to_value(lods)?);
        }

        // Aggregate city object types
        let all_types: HashSet<String> = items
            .iter()
            .flat_map(|item| get_string_array(item, "city3d:co_types"))
            .collect();
        if !all_types.is_empty() {
            let mut types: Vec<String> = all_types.into_iter().collect();
            types.sort();
            self.summaries
                .insert("city3d:co_types".to_string(), serde_json::to_value(types)?);
        }

        // City object count statistics
        let counts: Vec<i64> = items
            .iter()
            .filter_map(|item| get_int(item, "city3d:city_objects"))
            .collect();
        if !counts.is_empty() {
            let min = *counts.iter().min().unwrap();
            let max = *counts.iter().max().unwrap();
            let total: i64 = counts.iter().sum();

            let stats = serde_json::json!({
                "min": min,
                "max": max,
                "total": total
            });

            self.summaries
                .insert("city3d:city_objects".to_string(), stats);
        }

        // Aggregate boolean fields as arrays
        let semantic_values: HashSet<bool> = items
            .iter()
            .filter_map(|item| {
                item.properties
                    .additional_fields
                    .get("city3d:semantic_surfaces")
                    .and_then(|v| v.as_bool())
            })
            .collect();
        if !semantic_values.is_empty() {
            let mut vals: Vec<bool> = semantic_values.into_iter().collect();
            vals.sort();
            self.summaries.insert(
                "city3d:semantic_surfaces".to_string(),
                serde_json::to_value(vals)?,
            );
        }

        let texture_values: HashSet<bool> = items
            .iter()
            .filter_map(|item| {
                item.properties
                    .additional_fields
                    .get("city3d:textures")
                    .and_then(|v| v.as_bool())
            })
            .collect();
        if !texture_values.is_empty() {
            let mut vals: Vec<bool> = texture_values.into_iter().collect();
            vals.sort();
            self.summaries
                .insert("city3d:textures".to_string(), serde_json::to_value(vals)?);
        }

        let material_values: HashSet<bool> = items
            .iter()
            .filter_map(|item| {
                item.properties
                    .additional_fields
                    .get("city3d:materials")
                    .and_then(|v| v.as_bool())
            })
            .collect();
        if !material_values.is_empty() {
            let mut vals: Vec<bool> = material_values.into_iter().collect();
            vals.sort();
            self.summaries
                .insert("city3d:materials".to_string(), serde_json::to_value(vals)?);
        }

        // Aggregate proj:code
        let unique_proj_codes: HashSet<String> = items
            .iter()
            .filter_map(|item| get_string(item, "proj:code"))
            .collect();

        if !unique_proj_codes.is_empty() {
            let mut codes: Vec<String> = unique_proj_codes.into_iter().collect();
            codes.sort();
            self.summaries
                .insert("proj:code".to_string(), serde_json::to_value(codes)?);
        }

        // Aggregate data-asset file sizes (STAC File Extension `file:size`)
        let sizes: Vec<u64> = items
            .iter()
            .filter_map(|item| {
                item.assets
                    .get("data")
                    .and_then(|asset| asset.additional_fields.get("file:size"))
                    .and_then(|v| v.as_u64())
            })
            .collect();
        if !sizes.is_empty() {
            let size_min = sizes.iter().copied().min();
            let size_max = sizes.iter().copied().max();
            let size_sum: u128 = sizes.iter().map(|s| *s as u128).sum();
            if let Some(file_size) =
                file_size_summary(size_min, size_max, size_sum, sizes.len() as u64)
            {
                self.summaries.insert("file:size".to_string(), file_size);
            }
        }

        // Merge spatial extents from item bboxes. Per the GeoJSON/STAC bbox
        // spec, a bbox array has exactly 2*n elements for n dimensions, so
        // only 4 (2D) or 6 (3D) are valid here. A length in between or
        // beyond (e.g. 5 or 7) is malformed: silently reading it as 2D and
        // discarding the trailing element(s) would misinterpret their
        // meaning rather than report the problem, so such items are skipped
        // with a logged reason instead of aggregated.
        let parsed_bboxes: Vec<BBox3D> = items
            .iter()
            .filter_map(|item| {
                let bbox_vec: Vec<f64> = item.bbox.clone()?;
                match bbox_vec.len() {
                    6 => Some(BBox3D::new(
                        bbox_vec[0],
                        bbox_vec[1],
                        bbox_vec[2],
                        bbox_vec[3],
                        bbox_vec[4],
                        bbox_vec[5],
                    )),
                    4 => Some(BBox3D::new(
                        bbox_vec[0],
                        bbox_vec[1],
                        0.0,
                        bbox_vec[2],
                        bbox_vec[3],
                        0.0,
                    )),
                    n => {
                        log::warn!(
                            "item {:?} has a bbox with {n} element(s) (expected 4 or 6); \
                             skipping it for spatial extent aggregation",
                            item.id
                        );
                        None
                    }
                }
            })
            .collect();

        if !parsed_bboxes.is_empty() {
            let mut merged = parsed_bboxes[0].clone();
            for bbox in &parsed_bboxes[1..] {
                merged = merged.merge(bbox);
            }
            self = self.spatial_extent(merged);
        }

        Ok(self)
    }

    /// Build the STAC Collection
    pub fn build(self) -> Result<stac::Collection> {
        // Validate spatial extent
        if self.spatial_bboxes.is_empty() {
            return Err(CityJsonStacError::StacError(
                "Spatial extent bbox is required".to_string(),
            ));
        }

        let description = self
            .description
            .unwrap_or_else(|| "3D City Model collection".to_string());

        let mut collection = stac::Collection::new(&self.id, &description);
        collection.title = self.title;
        collection.license = self.license;
        collection.keywords = self.keywords;
        collection.providers = self.providers;

        // Set extent
        collection.extent.spatial.bbox = self.spatial_bboxes;
        collection.extent.temporal.interval = vec![[self.temporal_start, self.temporal_end]];

        // Set summaries
        if !self.summaries.is_empty() {
            let mut map = Map::new();
            for (k, v) in self.summaries.iter() {
                map.insert(k.clone(), v.clone());
            }
            collection.summaries = Some(map);
        }

        // Set links
        collection.links = self.links;

        // Set assets
        collection.assets = self.assets;

        // Build stac_extensions list
        let mut stac_extensions = vec![CITY3D_EXTENSION.to_string()];

        if self.summaries.contains_key("proj:code") {
            stac_extensions.push(PROJECTION_EXTENSION.to_string());
        }

        // Note: city3d:city_objects statistics (min/max/total) are defined by the
        // city3d extension itself, not the STAC Stats Extension

        // Auto-generate item_assets if not explicitly set
        if self.item_assets.is_empty() && self.summaries.contains_key("city3d:version") {
            let mut additional_fields = Map::new();

            // Add proj:code to item_assets when a single CRS is known
            if let Some(proj_codes) = self.summaries.get("proj:code") {
                if let Some(arr) = proj_codes.as_array() {
                    if arr.len() == 1 {
                        if let Some(code) = arr[0].as_str() {
                            additional_fields
                                .insert("proj:code".to_string(), Value::String(code.to_string()));
                        }
                    }
                }
            }

            // Add city3d extension properties to item_assets
            if let Some(lods) = self.summaries.get("city3d:lods") {
                additional_fields.insert("city3d:lods".to_string(), lods.clone());
            }
            if let Some(co_types) = self.summaries.get("city3d:co_types") {
                additional_fields.insert("city3d:co_types".to_string(), co_types.clone());
            }
            if let Some(version) = self.summaries.get("city3d:version") {
                // For item_assets, use a single version string if only one exists
                if let Some(arr) = version.as_array() {
                    if arr.len() == 1 {
                        additional_fields.insert("city3d:version".to_string(), arr[0].clone());
                    }
                }
            }

            let item_asset = stac::ItemAsset {
                title: Some("3D city model data".to_string()),
                description: None,
                r#type: None,
                roles: vec!["data".to_string()],
                additional_fields,
            };
            collection
                .item_assets
                .insert("data".to_string(), item_asset);
            stac_extensions.push(ITEM_ASSETS_EXTENSION.to_string());
        } else if !self.item_assets.is_empty() {
            collection.item_assets = self.item_assets;
            stac_extensions.push(ITEM_ASSETS_EXTENSION.to_string());
        }

        // Add File Extension if file:size or file:checksum is used on any
        // collection-level asset, or if the collection has items (items carry
        // file:size on their data assets when built from local files)
        let has_file_props_on_assets = collection.assets.values().any(|a| {
            a.additional_fields.contains_key("file:size")
                || a.additional_fields.contains_key("file:checksum")
        });
        if has_file_props_on_assets || self.has_items {
            stac_extensions.push(FILE_EXTENSION.to_string());
        }

        collection.extensions = stac_extensions;

        Ok(collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::CityJSONReader;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_cityjson(version: &str, lod: &str, obj_type: &str) -> NamedTempFile {
        let mut temp_file = NamedTempFile::new().unwrap();
        let cityjson = format!(
            r#"{{
            "type": "CityJSON",
            "version": "{}",
            "transform": {{
                "scale": [0.01, 0.01, 0.01],
                "translate": [100000, 200000, 0]
            }},
            "metadata": {{
                "geographicalExtent": [1.0, 2.0, 0.0, 10.0, 20.0, 30.0],
                "referenceSystem": "https://www.opengis.net/def/crs/EPSG/0/7415"
            }},
            "CityObjects": {{
                "obj1": {{
                    "type": "{}",
                    "geometry": [{{
                        "type": "Solid",
                        "lod": "{}",
                        "boundaries": [[[[0,0,0]]]]
                    }}]
                }}
            }},
            "vertices": [[0,0,0]]
        }}"#,
            version, obj_type, lod
        );

        writeln!(temp_file, "{}", cityjson).unwrap();
        temp_file.flush().unwrap();
        temp_file
    }

    #[test]
    fn test_collection_builder_basic() {
        let bbox = BBox3D::new(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);

        let collection = StacCollectionBuilder::new("test-collection")
            .title("Test Collection")
            .description("A test collection")
            .license("CC-BY-4.0")
            .spatial_extent(bbox)
            .build()
            .unwrap();

        assert_eq!(collection.id, "test-collection");
        assert_eq!(collection.title, Some("Test Collection".to_string()));
        assert_eq!(collection.license, "CC-BY-4.0");
        assert!(!collection.extent.spatial.bbox.is_empty());
    }

    #[test]
    fn test_summary_union_merges_arrays_and_dedupes() {
        let builder = StacCollectionBuilder::new("test-collection")
            .summary("city3d:lods", serde_json::json!(["1", "2"]))
            .summary_union("city3d:lods", serde_json::json!(["2", "3"]));

        let value = builder.summaries.get("city3d:lods").unwrap();
        let mut lods: Vec<&str> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        lods.sort();
        assert_eq!(lods, vec!["1", "2", "3"]);
    }

    #[test]
    fn test_summary_union_dedupes_number_and_string_representations() {
        let builder = StacCollectionBuilder::new("test-collection")
            .summary("city3d:lods", serde_json::json!([1, 2]))
            .summary_union("city3d:lods", serde_json::json!(["2", "3"]));

        let value = builder.summaries.get("city3d:lods").unwrap();
        let lods: Vec<&Value> = value.as_array().unwrap().iter().collect();
        // "2" (number) and "2" (string) dedupe to a single entry
        assert_eq!(lods.len(), 3);
    }

    #[test]
    fn test_summary_union_no_existing_value_falls_back_to_config() {
        let builder = StacCollectionBuilder::new("test-collection").summary_union(
            "city3d:co_types",
            serde_json::json!(["Building", "TINRelief"]),
        );

        let value = builder.summaries.get("city3d:co_types").unwrap();
        let types: Vec<&str> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(types, vec!["Building", "TINRelief"]);
    }

    #[test]
    fn test_summary_union_non_array_overwrites() {
        let builder = StacCollectionBuilder::new("test-collection")
            .summary(
                "city3d:city_objects",
                serde_json::json!({"min": 1, "max": 2, "total": 3}),
            )
            .summary_union(
                "city3d:city_objects",
                serde_json::json!({"min": 1, "max": 5, "total": 10}),
            );

        let value = builder.summaries.get("city3d:city_objects").unwrap();
        assert_eq!(value["max"], 5);
    }

    #[test]
    fn test_collection_aggregate_metadata() {
        let file1 = create_test_cityjson("2.0", "2", "Building");
        let file2 = create_test_cityjson("2.0", "3", "Road");

        let reader1 = CityJSONReader::new(file1.path()).unwrap();
        let reader2 = CityJSONReader::new(file2.path()).unwrap();

        let readers: Vec<Box<dyn CityModelMetadataReader>> =
            vec![Box::new(reader1), Box::new(reader2)];

        let collection = StacCollectionBuilder::new("test")
            .aggregate_cityjson_metadata(&readers)
            .unwrap()
            .build()
            .unwrap();

        let summaries = collection.summaries.unwrap();

        // Check aggregated LODs
        let lods = summaries.get("city3d:lods").unwrap().as_array().unwrap();
        assert_eq!(lods.len(), 2);

        // Check aggregated types
        let types = summaries
            .get("city3d:co_types")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(types.len(), 2);

        // Check city object stats
        let stats = summaries.get("city3d:city_objects").unwrap();
        assert_eq!(stats["total"], 2);
        assert_eq!(stats["min"], 1);
        assert_eq!(stats["max"], 1);
    }

    #[test]
    fn test_collection_file_size_summary_from_summaries() {
        let mut summaries = crate::stac::AggregatedSummaries::default();
        summaries.versions.insert("2.0".to_string());
        summaries.size_min = Some(1000);
        summaries.size_max = Some(3000);
        summaries.size_sum = 8000;
        summaries.size_count = 4;
        summaries.merged_bbox = Some(BBox3D::new(0.0, 0.0, 0.0, 10.0, 10.0, 10.0));

        let collection = StacCollectionBuilder::new("test")
            .aggregate_from_summaries(&summaries)
            .unwrap()
            .build()
            .unwrap();

        let file_size = collection.summaries.unwrap();
        let file_size = file_size.get("file:size").unwrap();
        assert_eq!(file_size["minimum"], 1000);
        assert_eq!(file_size["maximum"], 3000);
        assert_eq!(file_size["mean"], 2000); // 8000 / 4

        // File Extension declared
        assert!(collection
            .extensions
            .iter()
            .any(|e| e.contains("stac-extensions.github.io/file/")));
    }

    #[test]
    fn test_collection_file_size_summary_from_items() {
        let mut item = crate::stac::StacItem::new("i1");
        item.bbox = Some(vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0]);
        let mut asset = crate::stac::Asset::new("./data.json");
        asset
            .additional_fields
            .insert("file:size".to_string(), Value::Number(2048.into()));
        item.assets.insert("data".to_string(), asset);

        let collection = StacCollectionBuilder::new("test")
            .aggregate_from_items(&[item])
            .unwrap()
            .build()
            .unwrap();

        let summaries = collection.summaries.unwrap();
        let file_size = summaries.get("file:size").unwrap();
        assert_eq!(file_size["minimum"], 2048);
        assert_eq!(file_size["maximum"], 2048);
        assert_eq!(file_size["mean"], 2048);
    }

    #[test]
    fn test_collection_temporal_extent() {
        let bbox = BBox3D::new(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);
        let start = Utc::now();

        let collection = StacCollectionBuilder::new("test")
            .spatial_extent(bbox)
            .temporal_extent(Some(start), None)
            .build()
            .unwrap();

        assert!(!collection.extent.temporal.interval.is_empty());
        assert_eq!(collection.extent.temporal.interval.len(), 1);
    }
}
