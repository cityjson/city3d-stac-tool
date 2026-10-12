//! # City3D STAC
//!
//! A library for generating STAC (SpatioTemporal Asset Catalog) metadata
//! from 3D city model datasets. It backs the `city3dstac` CLI.
//!
//! The STAC Item vocabulary (`StacItemBuilder`, `City3dProperties`,
//! metadata types) lives in the dependency-light
//! [`city3d-stac-types`](https://docs.rs/city3d-stac-types) crate and is
//! re-exported from [`stac`] and [`metadata`].
//!
//! ## Supported Formats
//!
//! - CityJSON (`.json`)
//! - CityJSON Sequences (`.jsonl`)
//! - CityGML 2.0 / 3.0 (`.gml`, `.xml`)
//! - FlatCityBuf (`.fcb`)
//! - ZIP and GZIP archives containing the above (`.zip`, `.gz`)
//!
//! ## Example
//!
//! ```no_run
//! use city3d_stac::reader::get_reader;
//! use city3d_stac::stac::StacItemBuilder;
//! use std::path::Path;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let reader = get_reader(Path::new("building.json"))?;
//! let props = city3d_stac::adapter::properties_from_reader(&*reader)?;
//! let item = StacItemBuilder::new("my-building")
//!     .bbox(reader.bbox()?)
//!     .city3d(props)?
//!     .build()?;
//! # Ok(())
//! # }
//! ```

pub mod adapter;
pub mod cli;
pub mod config;
pub mod error;
pub mod memory;
pub mod metadata;
pub mod reader;
pub mod remote;
pub mod stac;
pub mod traversal;
pub mod validation;

pub use error::{CityJsonStacError, Result};
