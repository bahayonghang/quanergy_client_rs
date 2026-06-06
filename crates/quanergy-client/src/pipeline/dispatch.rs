use crate::{
    cloud::{Frame, PointHvdir},
    config::PipelineConfig,
    error::{QuanergyError, Result},
    protocol::{
        PacketHeader, PACKET_TYPE_HVDIR_LIST, PACKET_TYPE_M1, PACKET_TYPE_M_SERIES,
        PACKET_TYPE_M_SERIES_REDUCED,
    },
};

use super::{m_series::MSeriesParser, packet_01::parse_01};

pub(super) struct ParserDispatch {
    m_series: MSeriesParser,
}

impl ParserDispatch {
    pub(super) fn new(config: &PipelineConfig) -> Result<Self> {
        Ok(Self {
            m_series: MSeriesParser::new(config)?,
        })
    }

    pub(super) fn parse(&mut self, packet: &[u8]) -> Result<Vec<Frame<PointHvdir>>> {
        let header = PacketHeader::parse(packet)?;
        match header.packet_type {
            PACKET_TYPE_M_SERIES => self.m_series.parse_00(packet, header),
            PACKET_TYPE_HVDIR_LIST => parse_01(packet, header, &self.m_series.frame_id),
            PACKET_TYPE_M_SERIES_REDUCED => self.m_series.parse_04(packet, header),
            PACKET_TYPE_M1 => self.m_series.parse_06(packet, header),
            other => Err(QuanergyError::UnsupportedPacketType(other)),
        }
    }
}
