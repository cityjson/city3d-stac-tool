//! Conversions between this project's STAC document types and the upstream
//! `stac` crate's, used where gen still needs upstream behaviour — schema
//! validation and GeoParquet item tables.
//!
//! These live in the gen crate, never in the types crate: the types crate
//! must not depend on `stac`.

use city3d_stac_types::stac::types::Item;

/// Convert to an upstream `stac::Item` by round-tripping through JSON.
///
/// Both models serialise to the same STAC document, so JSON is the honest
/// interchange: any field this project does not model is preserved by
/// `additional_fields` rather than silently dropped by a field-by-field copy.
pub fn to_upstream(item: &Item) -> crate::error::Result<stac::Item> {
    let value = serde_json::to_value(item)?;
    Ok(serde_json::from_value(value)?)
}

/// Convert from an upstream `stac::Item`.
pub fn from_upstream(item: &stac::Item) -> crate::error::Result<Item> {
    let value = serde_json::to_value(item)?;
    Ok(serde_json::from_value(value)?)
}
