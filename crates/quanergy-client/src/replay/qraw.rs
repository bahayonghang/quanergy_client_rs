use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::Path,
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::{
    error::{QuanergyError, Result},
    protocol::RawPacket,
};

const QRAW_MAGIC: &[u8; 8] = b"QRAWv1\0\0";
const QRAW_RECORD_HEADER_LEN: usize = 12;

pub struct QrawWriter<W: Write> {
    writer: W,
}

impl QrawWriter<BufWriter<File>> {
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(BufWriter::new(File::create(path)?))
    }
}

impl<W: Write> QrawWriter<W> {
    pub fn new(mut writer: W) -> Result<Self> {
        writer.write_all(QRAW_MAGIC)?;
        Ok(Self { writer })
    }

    pub fn write_packet(&mut self, delta_ns: u64, packet: &[u8]) -> Result<()> {
        let len = u32::try_from(packet.len())
            .map_err(|_| QuanergyError::ReplayFormat("packet too large for qraw v1".to_owned()))?;
        self.writer.write_u64::<LittleEndian>(delta_ns)?;
        self.writer.write_u32::<LittleEndian>(len)?;
        self.writer.write_all(packet)?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

pub struct QrawReader<R: Read> {
    reader: R,
}

impl QrawReader<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(BufReader::new(File::open(path)?))
    }
}

impl<R: Read> QrawReader<R> {
    pub fn new(mut reader: R) -> Result<Self> {
        let mut magic = [0u8; QRAW_MAGIC.len()];
        reader.read_exact(&mut magic)?;
        if &magic != QRAW_MAGIC {
            return Err(QuanergyError::ReplayFormat("invalid qraw magic".to_owned()));
        }
        Ok(Self { reader })
    }

    pub fn next_packet(&mut self) -> Result<Option<RawPacket>> {
        let mut header = [0u8; QRAW_RECORD_HEADER_LEN];
        match self.reader.read_exact(&mut header) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(error) => return Err(error.into()),
        }

        let mut cursor = &header[..];
        let delta_ns = cursor.read_u64::<LittleEndian>()?;
        let len = cursor.read_u32::<LittleEndian>()? as usize;
        let mut bytes = vec![0u8; len];
        self.reader.read_exact(&mut bytes)?;
        Ok(Some(RawPacket::parse(bytes, delta_ns)?))
    }
}
