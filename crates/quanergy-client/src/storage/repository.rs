//! Database abstraction for storing frame-level metadata.
//!
//! This module declares the [`ScanFrameMetadataStore`] trait and supporting
//! types.  The trait is **intentionally not implemented** in this crate yet;
//! existing code continues to use the concrete [`crate::storage::SqliteStore`]
//! directly.
//!
//! # Relationship to PCD migration
//!
//! Once the PCD writer becomes the production default and richer metadata
//! columns exist in the database, a future task will add an `impl
//! ScanFrameMetadataStore for SqliteStore`.  Until then the trait serves as a
//! stable compilation contract that upper-layer code can be written against
//! without coupling to a specific backend.

use crate::{error::Result, storage::metadata::NewScanFrame};

/// Stable, backend-allocated identifier for a persisted frame row.
///
/// This newtype prevents caller coupling to SQLite's `last_insert_rowid()`
/// and allows future PostgreSQL / object-store backends to use their own id
/// schemes.
///
/// The inner `i64` is always positive for a successfully persisted frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScanFrameId(pub i64);

/// Persist frame-level metadata for a point cloud that has already been
/// written to disk and atomically committed to its final path.
///
/// # Contract
///
/// - The trait **does not** persist per-point data — only one metadata row
///   per frame.
/// - The pair `(session_id, sequence)` must be unique; duplicate saves
///   return an error rather than silently overwriting.
/// - On success the method returns a stable [`ScanFrameId`] that can be used
///   for cross-referencing.
/// - The method is **synchronous** and **blocking**.  The calling storage
///   worker thread is expected to be the one that absorbs I/O latency.
///
/// # Design notes
///
/// - `&mut self` — signals single-writer semantics, required for
///   sequential commit and future transaction support.
/// - `Send` bound — allows the store to be moved into a dedicated storage
///   worker thread.
/// - `Sync` is **not** required, matching `rusqlite::Connection` usage.
///
/// # Stability
///
/// This trait is not yet implemented.  It is a forward-looking API boundary;
/// existing code should continue using [`crate::storage::SqliteStore`]
/// directly until the implementation task arrives.
pub trait ScanFrameMetadataStore: Send {
    /// Save one frame's metadata after the point-cloud file has been
    /// atomically committed to its final path.
    ///
    /// # Errors
    ///
    /// - Must return an error when `(session_id, sequence)` already exists.
    /// - May return storage-backend errors (I/O, constraint violations,
    ///   etc.).
    fn save_scan_frame_metadata(&mut self, frame: &NewScanFrame) -> Result<ScanFrameId>;
}
