use std::{
    io::Read,
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

use crate::{
    error::{QuanergyError, Result},
    protocol::{
        PacketHeader, RawPacket, DEFAULT_TCP_PORT, DEVICE_INFO_PATH, DEVICE_INFO_PORT, HEADER_LEN,
    },
};

pub struct TcpPacketSource {
    stream: TcpStream,
    last_packet_at: Option<Instant>,
}

impl TcpPacketSource {
    pub fn connect(host: &str) -> Result<Self> {
        Self::connect_port(host, DEFAULT_TCP_PORT)
    }

    pub fn connect_port(host: &str, port: u16) -> Result<Self> {
        let addr = (host, port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| QuanergyError::Config(format!("could not resolve {host}:{port}")))?;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self {
            stream,
            last_packet_at: None,
        })
    }

    pub fn next_packet(&mut self) -> Result<RawPacket> {
        let mut header_bytes = [0u8; HEADER_LEN];
        self.stream.read_exact(&mut header_bytes)?;
        let header = PacketHeader::read_from(&header_bytes[..])?;
        let total_len = header.size as usize;
        if total_len < HEADER_LEN {
            return Err(QuanergyError::PacketSizeMismatch {
                expected: HEADER_LEN,
                actual: total_len,
            });
        }

        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(&header_bytes);
        bytes.resize(total_len, 0);
        self.stream.read_exact(&mut bytes[HEADER_LEN..])?;

        let now = Instant::now();
        let delta = self
            .last_packet_at
            .map(|last| now.duration_since(last).as_nanos() as u64)
            .unwrap_or(0);
        self.last_packet_at = Some(now);
        RawPacket::parse(bytes, delta)
    }
}

pub fn fetch_device_info_xml(host: &str) -> Result<String> {
    let url = format!("http://{host}:{DEVICE_INFO_PORT}{DEVICE_INFO_PATH}");
    let response = ureq::get(&url)
        .call()
        .map_err(|error| QuanergyError::Http(error.to_string()))?;
    response
        .into_body()
        .read_to_string()
        .map_err(|error| QuanergyError::Http(error.to_string()))
}
