use std::path::PathBuf;

use byteorder::{BigEndian, WriteBytesExt};
use quanergy_client::{
    config::PipelineConfig,
    pipeline::SensorPipeline,
    protocol::{PacketHeader, HEADER_LEN, PACKET_TYPE_HVDIR_LIST, SIGNATURE},
    replay::{QrawReader, QrawWriter},
    visualizer::{RerunSink, VisualizerSink},
};

#[test]
fn replay_visualizer_saves_nonempty_rrd() {
    let dir = target_temp_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let qraw_path = dir.join("capture.qraw");
    let rrd_path = dir.join("capture.rrd");

    {
        let mut writer = QrawWriter::create(&qraw_path).unwrap();
        writer.write_packet(0, &packet_01()).unwrap();
        writer.flush().unwrap();
    }

    let mut reader = QrawReader::open(&qraw_path).unwrap();
    let mut pipeline = SensorPipeline::new(PipelineConfig::default()).unwrap();
    let mut sink = RerunSink::save(&rrd_path).unwrap();
    while let Some(packet) = reader.next_packet().unwrap() {
        for frame in pipeline.process_raw(&packet).unwrap() {
            sink.log_frame(&frame).unwrap();
        }
    }
    sink.flush_blocking().unwrap();

    assert!(std::fs::metadata(&rrd_path).unwrap().len() > 0);
}

fn target_temp_dir() -> PathBuf {
    let unique = format!(
        "replay-visualizer-smoke-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    PathBuf::from("target").join(unique)
}

fn packet_01() -> Vec<u8> {
    let mut body = Vec::new();
    body.write_u32::<BigEndian>(7).unwrap();
    body.write_u32::<BigEndian>(0).unwrap();
    body.write_u32::<BigEndian>(2).unwrap();
    body.write_u32::<BigEndian>(0).unwrap();
    write_hvdir_point(&mut body, 0, 0, 2_000_000, 42);
    write_hvdir_point(&mut body, 1_000, 100, 3_000_000, 84);

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
    packet
}

fn write_hvdir_point(
    body: &mut Vec<u8>,
    horizontal_angle: i16,
    vertical_angle: i16,
    range: u32,
    intensity: u16,
) {
    body.write_i16::<BigEndian>(horizontal_angle).unwrap();
    body.write_i16::<BigEndian>(vertical_angle).unwrap();
    body.write_u32::<BigEndian>(range).unwrap();
    body.write_u16::<BigEndian>(intensity).unwrap();
    body.write_u8(0).unwrap();
    body.write_u8(0).unwrap();
}
