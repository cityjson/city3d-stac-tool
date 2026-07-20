//! Re-export of the metadata vocabulary, which now lives in
//! `city3d-stac-types`. Kept so existing `crate::metadata::…` paths and the
//! public `city3d_stac::metadata` API continue to resolve.

pub use city3d_stac_types::metadata::*;
