//! STAC Item builder

use crate::error::Result;
use crate::extensions::{CITY3D_EXTENSION, FILE_EXTENSION, PROJECTION_EXTENSION};
use crate::metadata::BBox3D;
use crate::metadata::CRS;
use crate::stac::types::{Asset, Item, Link};
use crate::stac::City3dProperties;
use chrono::{DateTime, Utc};
use indexmap::IndexMap;
use serde_json::Value;

/// Builder for STAC Items
pub struct StacItemBuilder {
    id: String,
    bbox: Option<Vec<f64>>,
    geometry: Option<Value>,
    properties: serde_json::Map<String, Value>,
    datetime: Option<DateTime<Utc>>,
    start_datetime: Option<DateTime<Utc>>,
    end_datetime: Option<DateTime<Utc>>,
    title: Option<String>,
    description: Option<String>,
    assets: IndexMap<String, Asset>,
    links: Vec<Link>,
    /// Track if File Extension is used (for stac_extensions list)
    uses_file_extension: bool,
    /// Collection ID (set when item belongs to a collection)
    collection_id: Option<String>,
}

impl StacItemBuilder {
    /// Create a new STAC Item builder
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            bbox: None,
            geometry: None,
            properties: serde_json::Map::new(),
            datetime: None,
            start_datetime: None,
            end_datetime: None,
            title: None,
            description: None,
            assets: IndexMap::new(),
            links: Vec::new(),
            uses_file_extension: false,
            collection_id: None,
        }
    }

    /// Set the 3D bounding box
    pub fn bbox(mut self, bbox: BBox3D) -> Self {
        self.bbox = Some(bbox.to_array().to_vec());
        self
    }

    /// Set the 2D geometry (footprint)
    pub fn geometry(mut self, geometry: Value) -> Self {
        self.geometry = Some(geometry);
        self
    }

    /// Set datetime as an RFC3339 string, or null if None
    pub fn datetime(mut self, dt: Option<String>) -> Self {
        self.datetime = dt.and_then(|s| s.parse::<DateTime<Utc>>().ok());
        self
    }

    /// Set start_datetime (used when datetime is null and a date range is specified)
    pub fn start_datetime(mut self, dt: impl Into<String>) -> Self {
        self.start_datetime = dt.into().parse::<DateTime<Utc>>().ok();
        self
    }

    /// Set end_datetime (used when datetime is null and a date range is specified)
    pub fn end_datetime(mut self, dt: impl Into<String>) -> Self {
        self.end_datetime = dt.into().parse::<DateTime<Utc>>().ok();
        self
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

    /// Add a custom property
    pub fn property(mut self, key: impl Into<String>, value: Value) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    /// Add 3D City Models extension properties.
    ///
    /// Uses the STAC 3D City Models Extension (`city3d:` prefix)
    /// <https://cityjson.github.io/stac-city3d/v0.2.0/schema.json>
    pub fn city3d(mut self, props: City3dProperties) -> Result<Self> {
        props.write_into(&mut self.properties)?;
        Ok(self)
    }

    /// Set `proj:code` from the dataset CRS (STAC Projection Extension v2.0.0).
    pub fn crs(mut self, crs: &CRS) -> Self {
        if let Some(proj_code) = crs.to_stac_proj_code() {
            self.properties
                .insert("proj:code".to_string(), Value::String(proj_code));
        }
        self
    }

    /// Set `datetime` from a CityJSON header `metadata` object's
    /// `referenceDate`, which is typically `YYYY-MM-DD`.
    pub fn datetime_from_reference_date(mut self, metadata: Option<&Value>) -> Self {
        if let Some(ref_date) = metadata
            .and_then(|m| m.get("referenceDate"))
            .and_then(|v| v.as_str())
        {
            let datetime_str = if ref_date.contains('T') {
                ref_date.to_string()
            } else {
                format!("{ref_date}T00:00:00Z")
            };
            self.datetime = datetime_str.parse::<DateTime<Utc>>().ok();
        }
        self
    }

    /// Add a data asset pointing to the source file
    ///
    /// Optionally accepts a file size and checksum which are placed on the asset as
    /// `file:size` and `file:checksum` per the STAC File Extension spec (file extension
    /// fields belong on assets, not item properties).
    pub fn data_asset(
        mut self,
        href: impl Into<String>,
        media_type: &str,
        file_size: Option<u64>,
        file_checksum: Option<String>,
    ) -> Self {
        let mut asset = Asset::new(href.into());
        asset.media_type = Some(media_type.to_string());
        asset.title = Some("3D city model data".to_string());
        asset.roles = vec!["data".to_string()];

        if let Some(size) = file_size {
            asset
                .additional_fields
                .insert("file:size".to_string(), Value::Number(size.into()));
            self.uses_file_extension = true;
        }

        if let Some(checksum) = file_checksum {
            asset
                .additional_fields
                .insert("file:checksum".to_string(), Value::String(checksum));
            self.uses_file_extension = true;
        }

        self.assets.insert("data".to_string(), asset);
        self
    }

    /// Add a custom asset
    pub fn asset(mut self, key: impl Into<String>, asset: Asset) -> Self {
        self.assets.insert(key.into(), asset);
        self
    }

    /// Add a link
    pub fn link(mut self, link: Link) -> Self {
        self.links.push(link);
        self
    }

    /// Add a self link
    pub fn self_link(mut self, href: impl ToString) -> Self {
        self.links.push(Link::self_(href));
        self
    }

    /// Add a parent link
    pub fn parent_link(mut self, href: impl ToString) -> Self {
        self.links.push(Link::parent(href));
        self
    }

    /// Set the collection ID this item belongs to
    pub fn collection_id(mut self, id: impl Into<String>) -> Self {
        self.collection_id = Some(id.into());
        self
    }

    /// Add a collection link
    pub fn collection_link(mut self, href: impl ToString) -> Self {
        self.links.push(Link::collection(href));
        self
    }

    /// Build the STAC Item
    pub fn build(self) -> Result<Item> {
        let mut item = Item::new(&self.id);

        // Set datetime fields
        item.properties.datetime = self.datetime;
        item.properties.start_datetime = self.start_datetime;
        item.properties.end_datetime = self.end_datetime;
        item.properties.title = self.title;
        item.properties.description = self.description;

        // Extension properties go in properties.additional_fields
        item.properties.additional_fields = self.properties;

        item.bbox = self.bbox;
        item.geometry = self.geometry;
        item.assets = self.assets;
        item.collection = self.collection_id;
        item.links = self.links;

        // Build stac_extensions list dynamically. Each extension is only
        // declared when the Item actually carries a field it governs — the
        // city3d schema's `require_any_field` rule requires at least one
        // `city3d:*` property whenever the extension URL is declared, so
        // declaring it unconditionally could produce a schema-invalid Item.
        let mut stac_extensions = Vec::new();

        if item
            .properties
            .additional_fields
            .keys()
            .any(|k| k.starts_with("city3d:"))
        {
            stac_extensions.push(CITY3D_EXTENSION.to_string());
        }

        if item.properties.additional_fields.contains_key("proj:code") {
            stac_extensions.push(PROJECTION_EXTENSION.to_string());
        }

        if self.uses_file_extension {
            stac_extensions.push(FILE_EXTENSION.to_string());
        }

        item.extensions = stac_extensions;

        Ok(item)
    }

    /// Generate a simple 2D polygon geometry from bbox
    pub fn geometry_from_bbox(mut self) -> Self {
        if let Some(ref bbox) = self.bbox {
            if bbox.len() >= 4 {
                let xmin = bbox[0];
                let ymin = bbox[1];
                let xmax = if bbox.len() == 6 { bbox[3] } else { bbox[2] };
                let ymax = if bbox.len() == 6 { bbox[4] } else { bbox[3] };

                let geometry = serde_json::json!({
                    "type": "Polygon",
                    "coordinates": [[
                        [xmin, ymin],
                        [xmax, ymin],
                        [xmax, ymax],
                        [xmin, ymax],
                        [xmin, ymin]
                    ]]
                });

                self.geometry = Some(geometry);
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::BBox3D;

    #[test]
    fn test_item_builder_basic() {
        let item = StacItemBuilder::new("test-item")
            .title("Test Item")
            .description("A test item")
            .build()
            .unwrap();

        assert_eq!(item.id, "test-item");
        assert_eq!(item.properties.title, Some("Test Item".to_string()));
    }

    #[test]
    fn test_geometry_from_bbox() {
        let bbox = BBox3D::new(0.0, 0.0, 0.0, 10.0, 10.0, 10.0);

        let item = StacItemBuilder::new("test")
            .bbox(bbox)
            .geometry_from_bbox()
            .build()
            .unwrap();

        assert!(item.geometry.is_some());
        let geom_value = serde_json::to_value(&item.geometry).unwrap();
        assert_eq!(geom_value["type"], "Polygon");
    }
}
