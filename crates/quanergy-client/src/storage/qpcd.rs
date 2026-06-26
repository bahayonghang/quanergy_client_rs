use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};

use crate::{
    cloud::{Frame, PointXyzir},
    error::{QuanergyError, Result},
};

const QPCD_MAGIC: &[u8; 8] = b"QPCDv1\0\0";
pub const QPCD_POINT_STRIDE: u32 = 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QpcdHeader {
    pub format_version: u8,
    pub point_stride: u32,
    pub point_count: u64,
    pub stamp_micros: u64,
    pub sequence: u64,
    pub coord_frame: String,
    pub frame_id: String,
    pub width: usize,
    pub height: usize,
    pub is_dense: bool,
    // --- v2 provenance fields (all optional for backward compat) ---
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_frame: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_frame: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub station_config_sha256: Option<String>,
}

impl QpcdHeader {
    pub fn from_frame(frame: &Frame<PointXyzir>, coord_frame: impl Into<String>) -> Self {
        Self {
            format_version: 1,
            point_stride: QPCD_POINT_STRIDE,
            point_count: frame.points.len() as u64,
            stamp_micros: frame.stamp_micros,
            sequence: frame.sequence,
            coord_frame: coord_frame.into(),
            frame_id: frame.frame_id.clone(),
            width: frame.width,
            height: frame.height,
            is_dense: frame.is_dense,
            source_frame: None,
            target_frame: None,
            station_id: None,
            transform_id: None,
            station_config_sha256: None,
        }
    }

    /// Create a header with provenance metadata.
    pub fn from_frame_with_provenance(
        frame: &Frame<PointXyzir>,
        coord_frame: impl Into<String>,
        source_frame: Option<String>,
        target_frame: Option<String>,
        station_id: Option<String>,
        transform_id: Option<String>,
        station_config_sha256: Option<String>,
    ) -> Self {
        Self {
            source_frame,
            target_frame,
            station_id,
            transform_id,
            station_config_sha256,
            ..Self::from_frame(frame, coord_frame)
        }
    }
}

#[deprecated(
    since = "0.2.0",
    note = "use storage::write_pcd instead; QPCD is superseded by PCD 0.7"
)]
pub fn write_qpcd(
    path: impl AsRef<Path>,
    frame: &Frame<PointXyzir>,
    coord_frame: impl Into<String>,
) -> Result<QpcdHeader> {
    let path = path.as_ref();
    create_parent_dir(path)?;
    let header = QpcdHeader::from_frame(frame, coord_frame);
    let tmp_path = tmp_path(path);
    {
        let mut writer = BufWriter::new(File::create(&tmp_path)?);
        write_qpcd_to_writer(&mut writer, &header, &frame.points)?;
        writer.flush()?;
    }
    match fs::rename(&tmp_path, path) {
        Ok(()) => {}
        Err(error) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(&tmp_path, path).map_err(|_| error)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(header)
}

/// Write a QPCD file with full provenance metadata.
///
/// The `source_frame`, `target_frame`, `station_id`, `transform_id`,
/// and `station_config_sha256` fields are recorded in the JSON header
/// for auditability. Old readers will ignore these optional fields.
#[allow(clippy::too_many_arguments)]
#[deprecated(
    since = "0.2.0",
    note = "use storage::write_pcd instead; QPCD is superseded by PCD 0.7"
)]
pub fn write_qpcd_with_metadata(
    path: impl AsRef<Path>,
    frame: &Frame<PointXyzir>,
    coord_frame: impl Into<String>,
    source_frame: Option<String>,
    target_frame: Option<String>,
    station_id: Option<String>,
    transform_id: Option<String>,
    station_config_sha256: Option<String>,
) -> Result<QpcdHeader> {
    let path = path.as_ref();
    create_parent_dir(path)?;
    let header = QpcdHeader::from_frame_with_provenance(
        frame,
        coord_frame,
        source_frame,
        target_frame,
        station_id,
        transform_id,
        station_config_sha256,
    );
    let tmp_path = tmp_path(path);
    {
        let mut writer = BufWriter::new(File::create(&tmp_path)?);
        write_qpcd_to_writer(&mut writer, &header, &frame.points)?;
        writer.flush()?;
    }
    match fs::rename(&tmp_path, path) {
        Ok(()) => {}
        Err(error) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(&tmp_path, path).map_err(|_| error)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(header)
}

fn create_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub fn read_qpcd(path: impl AsRef<Path>) -> Result<(QpcdHeader, Frame<PointXyzir>)> {
    let mut reader = BufReader::new(File::open(path)?);
    read_qpcd_from_reader(&mut reader)
}

fn write_qpcd_to_writer<W: Write>(
    writer: &mut W,
    header: &QpcdHeader,
    points: &[PointXyzir],
) -> Result<()> {
    writer.write_all(QPCD_MAGIC)?;
    writer.write_u32::<LittleEndian>(0)?;
    let header_json = serde_json::to_vec(header)?;
    let header_len = u32::try_from(header_json.len()).map_err(|_| {
        QuanergyError::StorageFormat("qpcd header JSON is too large for v1".to_owned())
    })?;
    writer.write_u32::<LittleEndian>(header_len)?;
    writer.write_all(&header_json)?;
    for point in points {
        writer.write_f32::<LittleEndian>(point.x)?;
        writer.write_f32::<LittleEndian>(point.y)?;
        writer.write_f32::<LittleEndian>(point.z)?;
        writer.write_f32::<LittleEndian>(point.intensity)?;
        writer.write_u16::<LittleEndian>(point.ring)?;
        writer.write_u16::<LittleEndian>(0)?;
    }
    Ok(())
}

fn read_qpcd_from_reader<R: Read>(reader: &mut R) -> Result<(QpcdHeader, Frame<PointXyzir>)> {
    let mut magic = [0u8; QPCD_MAGIC.len()];
    reader.read_exact(&mut magic)?;
    if &magic != QPCD_MAGIC {
        return Err(QuanergyError::StorageFormat(
            "invalid qpcd magic".to_owned(),
        ));
    }

    let reserved = reader.read_u32::<LittleEndian>()?;
    if reserved != 0 {
        return Err(QuanergyError::StorageFormat(
            "unsupported qpcd reserved header flags".to_owned(),
        ));
    }
    let header_len = reader.read_u32::<LittleEndian>()? as usize;
    if header_len == 0 {
        return Err(QuanergyError::StorageFormat("empty qpcd header".to_owned()));
    }

    let mut header_json = vec![0u8; header_len];
    reader.read_exact(&mut header_json)?;
    let header: QpcdHeader = serde_json::from_slice(&header_json)?;
    if header.format_version != 1 {
        return Err(QuanergyError::StorageFormat(format!(
            "unsupported qpcd version {}",
            header.format_version
        )));
    }
    if header.point_stride != QPCD_POINT_STRIDE {
        return Err(QuanergyError::StorageFormat(format!(
            "unsupported qpcd point stride {}",
            header.point_stride
        )));
    }

    let point_count = usize::try_from(header.point_count).map_err(|_| {
        QuanergyError::StorageFormat("qpcd point count does not fit in memory".to_owned())
    })?;
    let mut points = Vec::with_capacity(point_count);
    for _ in 0..point_count {
        points.push(PointXyzir {
            x: reader.read_f32::<LittleEndian>()?,
            y: reader.read_f32::<LittleEndian>()?,
            z: reader.read_f32::<LittleEndian>()?,
            intensity: reader.read_f32::<LittleEndian>()?,
            ring: reader.read_u16::<LittleEndian>()?,
        });
        let _flags = reader.read_u16::<LittleEndian>()?;
    }

    let frame = Frame {
        stamp_micros: header.stamp_micros,
        sequence: header.sequence,
        frame_id: header.frame_id.clone(),
        width: header.width,
        height: header.height,
        is_dense: header.is_dense,
        points,
    };
    Ok((header, frame))
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    PathBuf::from(tmp)
}
