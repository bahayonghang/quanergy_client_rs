use byteorder::{BigEndian, WriteBytesExt};

use super::*;
use crate::protocol::{
    PacketHeader, HEADER_LEN, M_SERIES_FIRINGS_PER_PACKET, M_SERIES_NUM_LASERS,
    M_SERIES_NUM_RETURNS, PACKET_TYPE_HVDIR_LIST, PACKET_TYPE_M1, PACKET_TYPE_M_SERIES,
    PACKET_TYPE_M_SERIES_REDUCED, SIGNATURE,
};

#[test]
fn packet_01_parses_hvdir_points() {
    let mut body = Vec::new();
    body.write_u32::<BigEndian>(7).unwrap();
    body.write_u32::<BigEndian>(0).unwrap();
    body.write_u32::<BigEndian>(1).unwrap();
    body.write_u32::<BigEndian>(0).unwrap();
    body.write_i16::<BigEndian>(0).unwrap();
    body.write_i16::<BigEndian>(0).unwrap();
    body.write_u32::<BigEndian>(2_000_000).unwrap();
    body.write_u16::<BigEndian>(42).unwrap();
    body.write_u8(0).unwrap();
    body.write_u8(0).unwrap();

    let mut packet = Vec::new();
    PacketHeader {
        signature: SIGNATURE,
        size: (HEADER_LEN + body.len()) as u32,
        seconds: 10,
        nanoseconds: 20_000,
        version_major: 0,
        version_minor: 1,
        version_patch: 0,
        packet_type: PACKET_TYPE_HVDIR_LIST,
    }
    .write_to(&mut packet)
    .unwrap();
    packet.extend(body);

    let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
    let frames = pipeline.process_packet_bytes(&packet).unwrap();
    assert_eq!(frames.len(), 1);
    assert!((frames[0].points[0].x - 2.0).abs() < 1e-6);
    assert_eq!(frames[0].points[0].intensity, 42.0);
}

#[test]
fn packet_00_parses_m_series_frame_on_wrap() {
    let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
    assert!(pipeline
        .process_packet_bytes(&packet_00_with_positions(5150))
        .unwrap()
        .is_empty());

    let frames = pipeline
        .process_packet_bytes(&packet_00_with_positions(5200))
        .unwrap();

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].height, M_SERIES_NUM_LASERS);
    assert_eq!(frames[0].width, M_SERIES_FIRINGS_PER_PACKET);
    assert_eq!(
        frames[0].points.len(),
        M_SERIES_FIRINGS_PER_PACKET * M_SERIES_NUM_LASERS
    );
    assert!(frames[0].points.iter().any(|point| point.intensity > 0.0));
}

#[test]
fn packet_04_parses_reduced_m_series_frame_on_wrap() {
    let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
    assert!(pipeline
        .process_packet_bytes(&packet_04_with_positions(5150))
        .unwrap()
        .is_empty());

    let frames = pipeline
        .process_packet_bytes(&packet_04_with_positions(5200))
        .unwrap();

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].height, M_SERIES_NUM_LASERS);
    assert_eq!(frames[0].width, M_SERIES_FIRINGS_PER_PACKET);
    assert_eq!(
        frames[0].points.len(),
        M_SERIES_FIRINGS_PER_PACKET * M_SERIES_NUM_LASERS
    );
}

#[test]
fn packet_06_parses_m1_frame_on_wrap() {
    let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
    assert!(pipeline
        .process_packet_bytes(&packet_06_with_positions(5150))
        .unwrap()
        .is_empty());

    let frames = pipeline
        .process_packet_bytes(&packet_06_with_positions(5200))
        .unwrap();

    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].height, 1);
    assert_eq!(frames[0].width, M_SERIES_FIRINGS_PER_PACKET);
    assert_eq!(frames[0].points.len(), M_SERIES_FIRINGS_PER_PACKET);
}

#[test]
fn invalid_packet_is_dropped_when_lenient() {
    let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
    let frames = pipeline.process_packet_bytes(&[0; HEADER_LEN]).unwrap();

    assert!(frames.is_empty());
    assert_eq!(pipeline.counters().bad_packets, 1);
}

#[test]
fn invalid_packet_fails_when_strict() {
    let mut pipeline = SensorPipeline::new(PipelineConfig {
        strict: true,
        ..PipelineConfig::default()
    })
    .unwrap();

    assert!(pipeline.process_packet_bytes(&[0; HEADER_LEN]).is_err());
}

fn packet_00_with_positions(start_position: u16) -> Vec<u8> {
    let mut body = Vec::new();
    for firing in 0..M_SERIES_FIRINGS_PER_PACKET {
        body.write_u16::<BigEndian>(start_position + firing as u16)
            .unwrap();
        body.write_u16::<BigEndian>(0).unwrap();
        for return_index in 0..M_SERIES_NUM_RETURNS {
            for laser in 0..M_SERIES_NUM_LASERS {
                let distance = if return_index == 0 {
                    200_000 + firing as u32 * 100 + laser as u32
                } else {
                    0
                };
                body.write_u32::<BigEndian>(distance).unwrap();
            }
        }
        for return_index in 0..M_SERIES_NUM_RETURNS {
            for laser in 0..M_SERIES_NUM_LASERS {
                let intensity = if return_index == 0 {
                    10 + laser as u8
                } else {
                    0
                };
                body.write_u8(intensity).unwrap();
            }
        }
        body.extend([0; M_SERIES_NUM_LASERS]);
    }
    body.write_u32::<BigEndian>(1).unwrap();
    body.write_u32::<BigEndian>(2_000).unwrap();
    body.write_u16::<BigEndian>(5).unwrap();
    body.write_u16::<BigEndian>(0).unwrap();
    packet_with_header(PACKET_TYPE_M_SERIES, body)
}

fn packet_04_with_positions(start_position: u16) -> Vec<u8> {
    let mut body = Vec::new();
    body.write_u16::<BigEndian>(0).unwrap();
    body.write_u8(0).unwrap();
    body.write_u8(0).unwrap();
    for firing in 0..M_SERIES_FIRINGS_PER_PACKET {
        body.write_u16::<BigEndian>(start_position + firing as u16)
            .unwrap();
        body.write_u16::<BigEndian>(0).unwrap();
        for laser in 0..M_SERIES_NUM_LASERS {
            body.write_u32::<BigEndian>(200_000 + firing as u32 * 100 + laser as u32)
                .unwrap();
        }
        for laser in 0..M_SERIES_NUM_LASERS {
            body.write_u8(20 + laser as u8).unwrap();
        }
    }
    packet_with_header(PACKET_TYPE_M_SERIES_REDUCED, body)
}

fn packet_06_with_positions(start_position: u16) -> Vec<u8> {
    let mut body = Vec::new();
    body.write_u16::<BigEndian>(0).unwrap();
    body.write_u8(0).unwrap();
    body.write_u8(0).unwrap();
    for firing in 0..M_SERIES_FIRINGS_PER_PACKET {
        body.write_u16::<BigEndian>(start_position + firing as u16)
            .unwrap();
        body.write_u16::<BigEndian>(0).unwrap();
        body.write_u32::<BigEndian>(200_000 + firing as u32 * 100)
            .unwrap();
        body.write_u8(30).unwrap();
        body.extend([0; 3]);
    }
    packet_with_header(PACKET_TYPE_M1, body)
}

fn packet_with_header(packet_type: u8, body: Vec<u8>) -> Vec<u8> {
    let mut packet = Vec::new();
    PacketHeader {
        signature: SIGNATURE,
        size: (HEADER_LEN + body.len()) as u32,
        seconds: 10,
        nanoseconds: 20_000,
        version_major: 0,
        version_minor: 1,
        version_patch: 0,
        packet_type,
    }
    .write_to(&mut packet)
    .unwrap();
    packet.extend(body);
    packet
}
