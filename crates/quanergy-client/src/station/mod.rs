//! Station coordinate module — configuration, geometry, and hammer layout.
//!
//! This module implements the tamping-station static model described in
//! `ref/plans/quanergy_tamping_station_y_axis_modification_plan.md`.

pub mod config;
pub mod geometry;
pub mod hammer;

pub use config::{
    load_station_config, parse_station_config, StationConfigError, ValidatedStationConfig,
};
pub use geometry::StationGeometry;
pub use hammer::{
    AxisAlignedRoi, HammerAssignment, HammerGeometry, HammerLayout, SupportedHammerAxis,
};
