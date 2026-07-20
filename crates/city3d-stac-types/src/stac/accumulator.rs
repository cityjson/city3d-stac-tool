//! Accumulator types for streaming collection generation
//!
//! These types allow accumulating minimal item metadata during streaming processing,
//! without keeping full item JSON in memory. Aggregates are computed incrementally
//! so that memory usage is O(1) regardless of item count.

use crate::metadata::{AttributeDefinition, BBox3D};
use crate::stac::types::Item;
use crate::stac::CityObjectsCount;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Minimal metadata extracted from a processed item for collection aggregation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemMetadata {
    pub id: String,
    pub bbox: Option<Vec<f64>>,
    pub city3d_version: Option<String>,
    pub city3d_city_objects: Option<CityObjectsCount>,
    pub city3d_lods: Option<Vec<String>>,
    pub city3d_co_types: Option<Vec<String>>,
    pub city3d_attributes: Option<Vec<AttributeDefinition>>,
    pub city3d_semantic_surfaces: Option<bool>,
    pub city3d_textures: Option<bool>,
    pub city3d_materials: Option<bool>,
    /// Projection code (e.g., "EPSG:7415")
    pub proj_code: Option<String>,
    /// Size in bytes of the data asset (STAC File Extension `file:size`)
    pub file_size: Option<u64>,
}

impl ItemMetadata {
    /// Extract minimal metadata from an [`Item`]
    pub fn from_item(item: &Item) -> Self {
        let props = &item.properties.additional_fields;

        let get_string = |key: &str| -> Option<String> {
            props.get(key).and_then(|v| v.as_str().map(String::from))
        };

        let get_bool = |key: &str| -> Option<bool> { props.get(key).and_then(|v| v.as_bool()) };

        let get_string_vec = |key: &str| -> Option<Vec<String>> {
            props.get(key).and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| match v {
                            serde_json::Value::String(s) => Some(s.clone()),
                            serde_json::Value::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                        .collect()
                })
            })
        };

        let city_objects = props.get("city3d:city_objects").and_then(|v| {
            if let Some(n) = v.as_u64() {
                Some(CityObjectsCount::Integer(n))
            } else if let Some(obj) = v.as_object() {
                let min = obj.get("min").and_then(|v| v.as_u64())?;
                let max = obj.get("max").and_then(|v| v.as_u64())?;
                let total = obj.get("total").and_then(|v| v.as_u64())?;
                Some(CityObjectsCount::Statistics { min, max, total })
            } else {
                None
            }
        });

        let attributes = props.get("city3d:attributes").and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
        });

        let bbox_vec = item.bbox.clone();

        // Extract file:size from the data asset (STAC File Extension fields live on assets)
        let file_size = item
            .assets
            .get("data")
            .and_then(|asset| asset.additional_fields.get("file:size"))
            .and_then(|v| v.as_u64());

        Self {
            id: item.id.clone(),
            bbox: bbox_vec,
            city3d_version: get_string("city3d:version"),
            city3d_city_objects: city_objects,
            city3d_lods: get_string_vec("city3d:lods"),
            city3d_co_types: get_string_vec("city3d:co_types"),
            city3d_attributes: attributes,
            city3d_semantic_surfaces: get_bool("city3d:semantic_surfaces"),
            city3d_textures: get_bool("city3d:textures"),
            city3d_materials: get_bool("city3d:materials"),
            proj_code: get_string("proj:code"),
            file_size,
        }
    }
}

/// Pre-aggregated summaries computed incrementally during item processing.
/// Memory usage is O(unique_values) not O(item_count).
#[derive(Debug, Clone, Default)]
pub struct AggregatedSummaries {
    pub versions: HashSet<String>,
    pub lods: HashSet<String>,
    pub co_types: HashSet<String>,
    pub proj_codes: HashSet<String>,
    pub semantic_surfaces: HashSet<bool>,
    pub textures: HashSet<bool>,
    pub materials: HashSet<bool>,
    pub count_min: Option<u64>,
    pub count_max: Option<u64>,
    pub count_total: u64,
    pub merged_bbox: Option<BBox3D>,
    /// Smallest data-asset file size observed (bytes).
    pub size_min: Option<u64>,
    /// Largest data-asset file size observed (bytes).
    pub size_max: Option<u64>,
    /// Sum of all observed file sizes (u128 to avoid overflow across many large files).
    pub size_sum: u128,
    /// Number of items that contributed a file size.
    pub size_count: u64,
}

impl AggregatedSummaries {
    /// Incrementally merge metadata from a single item.
    pub fn merge_item(&mut self, metadata: &ItemMetadata) {
        if let Some(v) = &metadata.city3d_version {
            self.versions.insert(v.clone());
        }
        if let Some(lods) = &metadata.city3d_lods {
            for lod in lods {
                self.lods.insert(lod.clone());
            }
        }
        if let Some(types) = &metadata.city3d_co_types {
            for t in types {
                self.co_types.insert(t.clone());
            }
        }
        if let Some(code) = &metadata.proj_code {
            self.proj_codes.insert(code.clone());
        }
        if let Some(v) = metadata.city3d_semantic_surfaces {
            self.semantic_surfaces.insert(v);
        }
        if let Some(v) = metadata.city3d_textures {
            self.textures.insert(v);
        }
        if let Some(v) = metadata.city3d_materials {
            self.materials.insert(v);
        }

        // Aggregate city object counts
        let count = match &metadata.city3d_city_objects {
            Some(CityObjectsCount::Integer(n)) => Some(*n),
            Some(CityObjectsCount::Statistics { total, .. }) => Some(*total),
            None => None,
        };
        if let Some(n) = count {
            self.count_min = Some(self.count_min.map_or(n, |m: u64| m.min(n)));
            self.count_max = Some(self.count_max.map_or(n, |m: u64| m.max(n)));
            self.count_total += n;
        }

        // Aggregate data-asset file sizes (STAC File Extension `file:size`)
        if let Some(size) = metadata.file_size {
            self.size_min = Some(self.size_min.map_or(size, |m: u64| m.min(size)));
            self.size_max = Some(self.size_max.map_or(size, |m: u64| m.max(size)));
            self.size_sum += size as u128;
            self.size_count += 1;
        }

        // Merge bbox
        if let Some(bbox_vec) = &metadata.bbox {
            let parsed = if bbox_vec.len() == 6 {
                Some(BBox3D::new(
                    bbox_vec[0],
                    bbox_vec[1],
                    bbox_vec[2],
                    bbox_vec[3],
                    bbox_vec[4],
                    bbox_vec[5],
                ))
            } else if bbox_vec.len() >= 4 {
                Some(BBox3D::new(
                    bbox_vec[0],
                    bbox_vec[1],
                    0.0,
                    bbox_vec[2],
                    bbox_vec[3],
                    0.0,
                ))
            } else {
                None
            };
            if let Some(bbox) = parsed {
                self.merged_bbox = Some(match self.merged_bbox.take() {
                    Some(existing) => existing.merge(&bbox),
                    None => bbox,
                });
            }
        }
    }
}

/// Accumulates metadata from multiple items for collection generation.
///
/// Uses incremental aggregation so that memory usage is bounded by the number of
/// unique values (versions, LODs, types, etc.) rather than the total item count.
#[derive(Debug, Clone, Default)]
pub struct CollectionAccumulator {
    pub summaries: AggregatedSummaries,
    pub item_links: Vec<(String, Option<String>)>,
    pub errors: Vec<(String, String)>,
    max_item_links: Option<usize>,
    omitted_item_links: usize,
    item_count: usize,
}

impl CollectionAccumulator {
    pub fn new(max_item_links: Option<usize>) -> Self {
        Self {
            max_item_links,
            ..Self::default()
        }
    }

    pub fn add_item(&mut self, metadata: ItemMetadata, href: String, title: Option<String>) {
        self.summaries.merge_item(&metadata);
        if self
            .max_item_links
            .is_none_or(|limit| self.item_links.len() < limit)
        {
            self.item_links.push((href, title));
        } else {
            self.omitted_item_links += 1;
        }
        self.item_count += 1;
    }

    pub fn add_error(&mut self, source: String, error: String) {
        self.errors.push((source, error));
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn successful_count(&self) -> usize {
        self.item_count
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn omitted_item_links(&self) -> usize {
        self.omitted_item_links
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn create_test_item() -> Item {
        let mut item = Item::new("test-item");
        item.bbox = Some(vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0]);
        item.properties.datetime = Some("2023-01-01T00:00:00Z".parse().unwrap());
        item.properties.additional_fields.insert(
            "city3d:version".to_string(),
            Value::String("2.0".to_string()),
        );
        item.properties
            .additional_fields
            .insert("city3d:city_objects".to_string(), Value::Number(42.into()));
        item.properties.additional_fields.insert(
            "city3d:lods".to_string(),
            Value::Array(vec![
                Value::String("LOD1".to_string()),
                Value::String("LOD2".to_string()),
            ]),
        );
        item.properties.additional_fields.insert(
            "city3d:co_types".to_string(),
            Value::Array(vec![Value::String("Building".to_string())]),
        );
        item.properties
            .additional_fields
            .insert("city3d:textures".to_string(), Value::Bool(true));
        item.properties
            .additional_fields
            .insert("city3d:materials".to_string(), Value::Bool(false));
        item.properties
            .additional_fields
            .insert("city3d:semantic_surfaces".to_string(), Value::Bool(true));

        let mut asset = crate::stac::types::Asset::new("./data.json");
        asset
            .additional_fields
            .insert("file:size".to_string(), Value::Number(1000.into()));
        item.assets.insert("data".to_string(), asset);

        item.links
            .push(crate::stac::types::Link::self_("./item.json"));

        item
    }

    #[test]
    fn test_item_metadata_from_item() {
        let item = create_test_item();
        let metadata = ItemMetadata::from_item(&item);

        assert_eq!(metadata.id, "test-item");
        assert_eq!(metadata.bbox, Some(vec![0.0, 0.0, 0.0, 10.0, 10.0, 10.0]));
        assert_eq!(metadata.city3d_version, Some("2.0".to_string()));
        assert_eq!(
            metadata.city3d_city_objects,
            Some(CityObjectsCount::Integer(42))
        );
        assert_eq!(
            metadata.city3d_lods,
            Some(vec!["LOD1".to_string(), "LOD2".to_string()])
        );
        assert_eq!(metadata.city3d_co_types, Some(vec!["Building".to_string()]));
        assert_eq!(metadata.city3d_textures, Some(true));
        assert_eq!(metadata.city3d_materials, Some(false));
        assert_eq!(metadata.city3d_semantic_surfaces, Some(true));
        assert_eq!(metadata.file_size, Some(1000));
    }

    #[test]
    fn test_file_size_aggregation() {
        let mut summaries = AggregatedSummaries::default();

        let mut m1 = ItemMetadata::from_item(&create_test_item());
        m1.file_size = Some(1000);
        summaries.merge_item(&m1);

        let mut m2 = ItemMetadata::from_item(&create_test_item());
        m2.file_size = Some(3000);
        summaries.merge_item(&m2);

        assert_eq!(summaries.size_min, Some(1000));
        assert_eq!(summaries.size_max, Some(3000));
        assert_eq!(summaries.size_sum, 4000);
        assert_eq!(summaries.size_count, 2);
    }

    #[test]
    fn test_collection_accumulator() {
        let mut accumulator = CollectionAccumulator::new(None);

        let item = create_test_item();
        let metadata = ItemMetadata::from_item(&item);

        accumulator.add_item(
            metadata,
            "./items/test-item.json".to_string(),
            Some("test-item".to_string()),
        );

        assert_eq!(accumulator.successful_count(), 1);
        assert_eq!(accumulator.error_count(), 0);
        assert!(!accumulator.has_errors());

        accumulator.add_error("failed.json".to_string(), "Parse error".to_string());

        assert_eq!(accumulator.successful_count(), 1);
        assert_eq!(accumulator.error_count(), 1);
        assert!(accumulator.has_errors());
    }

    #[test]
    fn test_incremental_aggregation() {
        let mut accumulator = CollectionAccumulator::new(None);

        // Add first item
        let item1 = create_test_item();
        let metadata1 = ItemMetadata::from_item(&item1);
        accumulator.add_item(
            metadata1,
            "./items/item1.json".to_string(),
            Some("item1".to_string()),
        );

        // Add second item with different values
        let mut item2 = Item::new("test-item-2");
        item2.bbox = Some(vec![5.0, 5.0, 5.0, 20.0, 20.0, 20.0]);
        item2.properties.datetime = Some("2023-01-01T00:00:00Z".parse().unwrap());
        item2.properties.additional_fields.insert(
            "city3d:version".to_string(),
            Value::String("2.0".to_string()),
        );
        item2
            .properties
            .additional_fields
            .insert("city3d:city_objects".to_string(), Value::Number(100.into()));
        item2.properties.additional_fields.insert(
            "city3d:lods".to_string(),
            Value::Array(vec![Value::String("LOD3".to_string())]),
        );

        let metadata2 = ItemMetadata::from_item(&item2);
        accumulator.add_item(
            metadata2,
            "./items/item2.json".to_string(),
            Some("item2".to_string()),
        );

        assert_eq!(accumulator.successful_count(), 2);

        let summaries = &accumulator.summaries;
        assert!(summaries.versions.contains("2.0"));
        assert!(summaries.lods.contains("LOD1"));
        assert!(summaries.lods.contains("LOD2"));
        assert!(summaries.lods.contains("LOD3"));
        assert_eq!(summaries.count_min, Some(42));
        assert_eq!(summaries.count_max, Some(100));
        assert_eq!(summaries.count_total, 142);

        // Merged bbox should be union: [0,0,0, 20,20,20]
        let bbox = summaries.merged_bbox.as_ref().unwrap();
        assert!((bbox.xmin - 0.0).abs() < f64::EPSILON);
        assert!((bbox.xmax - 20.0).abs() < f64::EPSILON);
    }
}
