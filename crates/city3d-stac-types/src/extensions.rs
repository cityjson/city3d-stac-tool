//! Pinned STAC extension schema URLs.
//!
//! These were previously hardcoded in four places. Centralising them means a
//! version bump is one edit, and the vendored-schema drift test can assert
//! the pin matches the schema this crate was written against.

/// The 3D City Models extension.
pub const CITY3D_EXTENSION: &str = "https://cityjson.github.io/stac-city3d/v0.2.0/schema.json";

/// The Projection extension.
pub const PROJECTION_EXTENSION: &str =
    "https://stac-extensions.github.io/projection/v2.0.0/schema.json";

/// The File extension.
pub const FILE_EXTENSION: &str = "https://stac-extensions.github.io/file/v2.1.0/schema.json";

/// The Item Assets extension (Collections only).
pub const ITEM_ASSETS_EXTENSION: &str =
    "https://stac-extensions.github.io/item-assets/v1.0.0/schema.json";
