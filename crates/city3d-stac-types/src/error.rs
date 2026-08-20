//! Errors raised while building STAC documents for 3D city models.
//!
//! Deliberately narrower than the gen crate's error type: this crate performs
//! no I/O, no HTTP and no archive handling, so it has no `io`, `url`,
//! `object_store` or `zip` variants.

use thiserror::Error;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, City3dError>;

/// Errors raised while building STAC documents for 3D city models.
#[derive(Error, Debug)]
pub enum City3dError {
    /// JSON serialisation or deserialisation failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Metadata could not be interpreted.
    #[error("Failed to extract metadata: {0}")]
    Metadata(String),

    /// A field required to build a valid document is absent.
    #[error("Missing required field: {0}")]
    MissingField(String),

    /// The document could not be assembled.
    #[error("STAC generation error: {0}")]
    Stac(String),

    /// GeoJSON geometry could not be built.
    #[error("GeoJSON error: {0}")]
    GeoJson(String),

    /// A bounding box could not be reprojected to WGS84.
    #[error("Reprojection error: {0}")]
    Reprojection(String),

    /// Anything else.
    #[error("{0}")]
    Other(String),
}

impl From<String> for City3dError {
    fn from(s: String) -> Self {
        City3dError::Other(s)
    }
}

impl From<&str> for City3dError {
    fn from(s: &str) -> Self {
        City3dError::Other(s.to_string())
    }
}
