//! STAC generation module
//!
//! Items use this project's own document model, defined in
//! `city3d-stac-types`, so that crate stays free of the upstream `stac`
//! dependency. Collections and Catalogs still come from the upstream `stac`
//! crate; [`interop`] bridges the two where an upstream Item is required
//! (schema validation, GeoParquet).

mod catalog;
mod collection;
pub mod from_file;
pub mod geoparquet;
pub mod interop;

pub use catalog::StacCatalogBuilder;
pub use city3d_stac_types::stac::{
    AggregatedSummaries, City3dProperties, CityObjectsCount, CollectionAccumulator, ItemMetadata,
    StacItemBuilder,
};
pub use collection::StacCollectionBuilder;
pub use from_file::{
    item_from_file, item_from_file_with_crs_override, item_from_file_with_format_suffix_and_crs,
};

// Item documents: this project's own model. `Asset` and `Link` are the
// document model's own types (see `city3d-stac-types`), consistent with
// `StacItemBuilder::asset()`/`link()`, which take these types, not the
// upstream `stac` crate's. The upstream `stac::Item` is reached through
// [`interop`], never constructed directly, and only where gen still needs
// upstream behaviour (schema validation, GeoParquet).
pub type StacItem = city3d_stac_types::stac::types::Item;
pub type Asset = city3d_stac_types::stac::types::Asset;
pub type Link = city3d_stac_types::stac::types::Link;

// Re-export upstream `stac` crate types that describe Collections and
// Catalogs, which — unlike Items — are still modelled on the upstream
// crate. `collection.rs`, `catalog.rs` and `geoparquet.rs` also refer to
// the upstream crate directly via `stac::` paths for anything not
// re-exported here, so every remaining upstream use stays visible at the
// call site.
pub type StacCollection = stac::Collection;
pub type StacCatalog = stac::Catalog;
pub type Provider = stac::Provider;
pub type Extent = stac::Extent;
pub type SpatialExtent = stac::SpatialExtent;
pub type TemporalExtent = stac::TemporalExtent;
pub type Bbox = stac::Bbox;
pub type ItemAsset = stac::ItemAsset;
