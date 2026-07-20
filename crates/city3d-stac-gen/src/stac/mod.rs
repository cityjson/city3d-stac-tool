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
    item_from_file, item_from_file_with_crs_override, item_from_file_with_format_suffix,
    item_from_file_with_format_suffix_and_crs,
};

// Item documents: this project's own model.
pub type StacItem = city3d_stac_types::stac::types::Item;
pub type ItemAssetEntry = city3d_stac_types::stac::types::Asset;
pub type ItemLink = city3d_stac_types::stac::types::Link;

// The upstream Item, still needed by the GeoParquet writer and by
// `stac-validate`. Reach it through [`interop`], not by constructing it.
pub type UpstreamItem = stac::Item;

// Re-export upstream stac crate types used throughout the codebase.
// Type aliases preserve backward compatibility with existing code.
pub type StacCollection = stac::Collection;
pub type StacCatalog = stac::Catalog;
pub type Asset = stac::Asset;
pub type Link = stac::Link;
pub type Provider = stac::Provider;
pub type Extent = stac::Extent;
pub type SpatialExtent = stac::SpatialExtent;
pub type TemporalExtent = stac::TemporalExtent;
pub type Bbox = stac::Bbox;
pub type ItemAsset = stac::ItemAsset;
