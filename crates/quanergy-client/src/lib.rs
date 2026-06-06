//! Rust functional rewrite of the Quanergy C++ client SDK data path.
//!
//! The public API uses idiomatic Rust names. A small set of aliases preserves
//! C++ SDK concept names where that helps migration:
//! [`PointHVDIR`] maps to [`cloud::PointHvdir`], and [`PointXYZIR`] maps to
//! [`cloud::PointXyzir`].

pub mod calibration;
pub mod cloud;
pub mod config;
pub mod error;
pub mod filters;
pub mod net;
pub mod pipeline;
pub mod protocol;
pub mod replay;

pub use cloud::{Frame, PointHvdir, PointXyzir};
pub use error::{QuanergyError, Result};

/// C++ SDK migration alias for `quanergy::PointHVDIR`.
pub type PointHVDIR = cloud::PointHvdir;

/// C++ SDK migration alias for `quanergy::PointXYZIR`.
pub type PointXYZIR = cloud::PointXyzir;
