use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use rusqlite::{params, types::Type, Connection, OptionalExtension};

use crate::{
    error::{QuanergyError, Result},
    storage::{CaptureSession, NewCaptureSession, NewScanFrame, ScanFrameRecord},
};

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path
            .as_ref()
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let store = Self {
            conn: Connection::open_in_memory()?,
        };
        store.init_schema()?;
        Ok(store)
    }

    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS capture_session (
                session_id        TEXT PRIMARY KEY,
                started_at        TEXT NOT NULL,
                ended_at          TEXT,
                sensor_host       TEXT NOT NULL,
                sensor_model      TEXT,
                sdk_version       TEXT NOT NULL,
                status            TEXT NOT NULL,
                notes             TEXT
            );

            CREATE TABLE IF NOT EXISTS scan_frame (
                frame_id          INTEGER PRIMARY KEY,
                session_id        TEXT NOT NULL,
                sequence          INTEGER NOT NULL,
                timestamp_micros  INTEGER NOT NULL,
                sensor_host       TEXT NOT NULL,
                sensor_model      TEXT,
                packet_type_mask  INTEGER,
                point_count       INTEGER NOT NULL,
                coord_frame       TEXT NOT NULL,
                transform_4x4     BLOB NOT NULL,
                transform_json    TEXT NOT NULL,
                calibration_json  TEXT NOT NULL,
                cloud_path        TEXT NOT NULL,
                qraw_path         TEXT,
                status            TEXT NOT NULL,
                created_at        TEXT NOT NULL,
                FOREIGN KEY(session_id) REFERENCES capture_session(session_id),
                UNIQUE(session_id, sequence)
            );

            CREATE INDEX IF NOT EXISTS idx_scan_frame_session_sequence
                ON scan_frame(session_id, sequence);
            "#,
        )?;
        Ok(())
    }

    pub fn insert_capture_session(&self, session: &NewCaptureSession) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO capture_session (
                session_id, started_at, sensor_host, sensor_model, sdk_version, status, notes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                session.session_id,
                session.started_at,
                session.sensor_host,
                session.sensor_model,
                session.sdk_version,
                session.status,
                session.notes,
            ],
        )?;
        Ok(())
    }

    pub fn finish_capture_session(
        &self,
        session_id: &str,
        ended_at: &str,
        status: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE capture_session SET ended_at = ?1, status = ?2 WHERE session_id = ?3",
            params![ended_at, status, session_id],
        )?;
        Ok(())
    }

    pub fn get_capture_session(&self, session_id: &str) -> Result<Option<CaptureSession>> {
        self.conn
            .query_row(
                r#"
                SELECT session_id, started_at, ended_at, sensor_host, sensor_model,
                       sdk_version, status, notes
                FROM capture_session
                WHERE session_id = ?1
                "#,
                params![session_id],
                |row| {
                    Ok(CaptureSession {
                        session_id: row.get(0)?,
                        started_at: row.get(1)?,
                        ended_at: row.get(2)?,
                        sensor_host: row.get(3)?,
                        sensor_model: row.get(4)?,
                        sdk_version: row.get(5)?,
                        status: row.get(6)?,
                        notes: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_scan_frame(&self, frame: &NewScanFrame) -> Result<i64> {
        let matrix = encode_matrix(frame.transform_4x4)?;
        self.conn.execute(
            r#"
            INSERT INTO scan_frame (
                session_id, sequence, timestamp_micros, sensor_host, sensor_model,
                packet_type_mask, point_count, coord_frame, transform_4x4, transform_json,
                calibration_json, cloud_path, qraw_path, status, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                frame.session_id,
                to_i64(frame.sequence)?,
                to_i64(frame.timestamp_micros)?,
                frame.sensor_host,
                frame.sensor_model,
                frame.packet_type_mask.map(i64::from),
                to_i64(frame.point_count)?,
                frame.coord_frame,
                matrix,
                frame.transform_json,
                frame.calibration_json,
                frame.cloud_path,
                frame.qraw_path,
                frame.status,
                frame.created_at,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_scan_frame(
        &self,
        session_id: &str,
        sequence: u64,
    ) -> Result<Option<ScanFrameRecord>> {
        self.conn
            .query_row(
                r#"
                SELECT frame_id, session_id, sequence, timestamp_micros, sensor_host, sensor_model,
                       packet_type_mask, point_count, coord_frame, transform_4x4, transform_json,
                       calibration_json, cloud_path, qraw_path, status, created_at
                FROM scan_frame
                WHERE session_id = ?1 AND sequence = ?2
                "#,
                params![session_id, to_i64(sequence)?],
                scan_frame_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_scan_frames(&self, session_id: &str) -> Result<Vec<ScanFrameRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT frame_id, session_id, sequence, timestamp_micros, sensor_host, sensor_model,
                   packet_type_mask, point_count, coord_frame, transform_4x4, transform_json,
                   calibration_json, cloud_path, qraw_path, status, created_at
            FROM scan_frame
            WHERE session_id = ?1
            ORDER BY sequence
            "#,
        )?;
        let rows = stmt.query_map(params![session_id], scan_frame_from_row)?;
        let mut frames = Vec::new();
        for row in rows {
            frames.push(row?);
        }
        Ok(frames)
    }
}

fn scan_frame_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScanFrameRecord> {
    let matrix_blob: Vec<u8> = row.get(9)?;
    let transform_4x4 = decode_matrix(&matrix_blob).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(9, Type::Blob, Box::new(error))
    })?;

    Ok(ScanFrameRecord {
        frame_id: row.get(0)?,
        session_id: row.get(1)?,
        sequence: from_i64(row.get(2)?, 2)?,
        timestamp_micros: from_i64(row.get(3)?, 3)?,
        sensor_host: row.get(4)?,
        sensor_model: row.get(5)?,
        packet_type_mask: row
            .get::<_, Option<i64>>(6)?
            .map(|value| {
                u32::try_from(value).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(6, Type::Integer, Box::new(error))
                })
            })
            .transpose()?,
        point_count: from_i64(row.get(7)?, 7)?,
        coord_frame: row.get(8)?,
        transform_4x4,
        transform_json: row.get(10)?,
        calibration_json: row.get(11)?,
        cloud_path: row.get(12)?,
        qraw_path: row.get(13)?,
        status: row.get(14)?,
        created_at: row.get(15)?,
    })
}

fn encode_matrix(matrix: [[f32; 4]; 4]) -> Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(16 * std::mem::size_of::<f32>());
    for row in matrix {
        for value in row {
            bytes.write_f32::<LittleEndian>(value)?;
        }
    }
    Ok(bytes)
}

fn decode_matrix(bytes: &[u8]) -> Result<[[f32; 4]; 4]> {
    if bytes.len() != 16 * std::mem::size_of::<f32>() {
        return Err(QuanergyError::StorageFormat(format!(
            "transform_4x4 blob has {} bytes, expected 64",
            bytes.len()
        )));
    }
    let mut cursor = bytes;
    let mut matrix = [[0.0f32; 4]; 4];
    for row in &mut matrix {
        for value in row {
            *value = cursor.read_f32::<LittleEndian>()?;
        }
    }
    Ok(matrix)
}

fn to_i64(value: u64) -> Result<i64> {
    i64::try_from(value)
        .map_err(|_| QuanergyError::StorageFormat(format!("{value} does not fit in SQLite i64")))
}

fn from_i64(value: i64, column: usize) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(column, Type::Integer, Box::new(error))
    })
}
