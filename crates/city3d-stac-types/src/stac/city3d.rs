//! The typed `city3d:*` property set.
//!
//! This is the single place the STAC 3D City Models extension's field list is
//! written down in Rust. Previously these eight fields were untyped
//! `properties.insert("city3d:…", …)` calls duplicated across the item
//! builder, the collection builder and the accumulator.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::Result;
use crate::metadata::AttributeDefinition;
use crate::stac::models::CityObjectsCount;

/// The `city3d:*` extension fields for a single dataset.
///
/// Every field is optional at the STAC level — the extension schema requires
/// only that *at least one* is present. Empty collections are omitted on
/// serialisation; a `Some(false)` boolean is kept, because "this dataset has
/// no textures" is a real assertion, not an absence of information.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct City3dProperties {
    pub version: Option<String>,
    pub lods: Vec<String>,
    pub co_types: Vec<String>,
    pub city_objects: Option<CityObjectsCount>,
    pub semantic_surfaces: Option<bool>,
    pub textures: Option<bool>,
    pub materials: Option<bool>,
    pub attributes: Vec<AttributeDefinition>,
}

impl City3dProperties {
    /// An empty property set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Write the populated fields into a STAC properties map, prefixed with
    /// `city3d:`. Absent and empty-collection fields are not written.
    pub fn write_into(&self, props: &mut Map<String, Value>) -> Result<()> {
        if let Some(version) = &self.version {
            props.insert("city3d:version".to_string(), Value::String(version.clone()));
        }
        if let Some(count) = &self.city_objects {
            props.insert(
                "city3d:city_objects".to_string(),
                serde_json::to_value(count)?,
            );
        }
        if !self.lods.is_empty() {
            props.insert("city3d:lods".to_string(), serde_json::to_value(&self.lods)?);
        }
        if !self.co_types.is_empty() {
            props.insert(
                "city3d:co_types".to_string(),
                serde_json::to_value(&self.co_types)?,
            );
        }
        if !self.attributes.is_empty() {
            props.insert(
                "city3d:attributes".to_string(),
                serde_json::to_value(&self.attributes)?,
            );
        }
        if let Some(v) = self.semantic_surfaces {
            props.insert("city3d:semantic_surfaces".to_string(), Value::Bool(v));
        }
        if let Some(v) = self.textures {
            props.insert("city3d:textures".to_string(), Value::Bool(v));
        }
        if let Some(v) = self.materials {
            props.insert("city3d:materials".to_string(), Value::Bool(v));
        }
        Ok(())
    }
}
