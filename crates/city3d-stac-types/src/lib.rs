//! Types and serialiser for STAC Items and Collections describing 3D city
//! models, using the `city3d` STAC extension.
//!
//! This crate is deliberately dependency-light: it has no async runtime, no
//! HTTP or object-store client, and no dependency on the upstream `stac`
//! crate. That is what lets a writer such as `cityparquet-rs` depend on it
//! without inheriting a CLI's dependency tree.

pub mod error;
pub mod extensions;
pub mod metadata;
pub mod stac;

pub use error::{City3dError, Result};
