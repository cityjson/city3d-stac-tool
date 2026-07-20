//! A minimal STAC document model.
//!
//! These structs exist so this crate does not depend on the upstream `stac`
//! crate, which pulls in `cql2` (and with it `geo`, `geozero`, `sqlparser`
//! and `jsonschema`) unconditionally, and whose `geoparquet` feature pins
//! arrow/parquet 57 — conflicting with `cityparquet-rs`'s arrow 58.
//!
//! Only the subset of STAC this project writes is modelled. The gen crate
//! converts to and from upstream `stac` types where it still needs them.
//!
//! Field order matters: `serde_json` is built with `preserve_order`, so the
//! declaration order below is the key order of the emitted JSON. It mirrors
//! `stac::Item` field for field, which is what keeps the golden fixtures
//! byte-identical across the swap.

use chrono::{DateTime, NaiveDateTime, Utc};
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

/// Deserialise a datetime the way upstream `stac` does: RFC 3339 first, then a
/// naive datetime (with optional fractional seconds) assumed to be UTC.
///
/// STAC mandates RFC 3339, but real-world Items in the wild omit the timezone.
/// Rejecting them here would make this crate refuse documents the tool has
/// always accepted, so the read path stays permissive while the write path
/// stays strict — serialisation is always RFC 3339 with `Z`.
fn deserialize_datetime_permissively<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    let Some(s) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None);
    };

    if let Ok(datetime) = DateTime::parse_from_rfc3339(&s) {
        return Ok(Some(datetime.to_utc()));
    }

    let (mut datetime, remainder) =
        NaiveDateTime::parse_and_remainder(&s, "%Y-%m-%dT%H:%M:%S").map_err(D::Error::custom)?;
    if remainder.starts_with('.') {
        datetime =
            NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f").map_err(D::Error::custom)?;
    }
    Ok(Some(datetime.and_utc()))
}

/// The STAC specification version these documents declare.
pub const STAC_VERSION: &str = "1.1.0";

/// A STAC Item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    #[serde(rename = "type")]
    pub type_: String,
    pub stac_version: String,
    #[serde(
        rename = "stac_extensions",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub extensions: Vec<String>,
    pub id: String,
    /// Serialised even when absent: STAC (and GeoJSON) require the key, and
    /// upstream `stac::Item` emits it unconditionally.
    #[serde(default)]
    pub geometry: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<Vec<f64>>,
    #[serde(default)]
    pub properties: ItemProperties,
    /// Serialised even when empty, as upstream does: `links` is required.
    #[serde(default)]
    pub links: Vec<Link>,
    /// Serialised even when empty, as upstream does: `assets` is required.
    #[serde(default)]
    pub assets: IndexMap<String, Asset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<String>,
    /// Top-level members that are not part of the Item specification. Keeping
    /// them means a document read in and written back out is not silently
    /// stripped of foreign members.
    #[serde(flatten)]
    pub additional_fields: Map<String, Value>,
}

impl Item {
    /// A new Item with the given id and no other content.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            type_: "Feature".to_string(),
            stac_version: STAC_VERSION.to_string(),
            extensions: Vec::new(),
            id: id.into(),
            geometry: None,
            bbox: None,
            properties: ItemProperties::default(),
            links: Vec::new(),
            assets: IndexMap::new(),
            collection: None,
            additional_fields: Map::new(),
        }
    }
}

/// An Item's `properties` object.
///
/// `datetime` is serialised even when null, because STAC requires the key to
/// be present on every Item.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ItemProperties {
    #[serde(default, deserialize_with = "deserialize_datetime_permissively")]
    pub datetime: Option<DateTime<Utc>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_datetime_permissively"
    )]
    pub start_datetime: Option<DateTime<Utc>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_datetime_permissively"
    )]
    pub end_datetime: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(flatten)]
    pub additional_fields: Map<String, Value>,
}

/// A STAC Asset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub href: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(flatten)]
    pub additional_fields: Map<String, Value>,
}

impl Asset {
    /// A new Asset at `href` with no other content.
    pub fn new(href: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            ..Default::default()
        }
    }

    /// Set the asset's media type.
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    /// Set the asset's title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the asset's roles.
    pub fn with_roles(mut self, roles: impl IntoIterator<Item = String>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }
}

/// A STAC Link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub href: String,
    pub rel: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Members that are not part of the Link specification — including the
    /// STAC API fields (`method`, `headers`, `body`, `merge`) this project
    /// never writes. Keeping them means a link read in and written back out is
    /// not silently stripped, which is what makes `interop` lossless.
    #[serde(flatten)]
    pub additional_fields: Map<String, Value>,
}

impl Link {
    /// A link with an explicit relation type.
    pub fn new(href: impl ToString, rel: impl Into<String>) -> Self {
        Self {
            href: href.to_string(),
            rel: rel.into(),
            media_type: None,
            title: None,
            additional_fields: Map::new(),
        }
    }

    /// Set the link's media type.
    pub fn with_media_type(mut self, media_type: Option<String>) -> Self {
        self.media_type = media_type;
        self
    }

    /// A `collection` link.
    pub fn collection(href: impl ToString) -> Self {
        Self::new(href, "collection")
    }

    /// A `parent` link.
    pub fn parent(href: impl ToString) -> Self {
        Self::new(href, "parent")
    }

    /// A `root` link.
    pub fn root(href: impl ToString) -> Self {
        Self::new(href, "root")
    }

    /// A `self` link.
    pub fn self_(href: impl ToString) -> Self {
        Self::new(href, "self")
    }

    /// An `item` link.
    pub fn item(href: impl ToString) -> Self {
        Self::new(href, "item")
    }

    /// A `child` link.
    pub fn child(href: impl ToString) -> Self {
        Self::new(href, "child")
    }
}
