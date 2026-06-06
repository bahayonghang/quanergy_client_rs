use crate::protocol::{PacketHeader, HEADER_LEN, PACKET_TYPE_HVDIR_LIST, SIGNATURE};

use super::*;

#[test]
fn qraw_round_trip_preserves_delta_and_packet() {
    let mut packet = Vec::new();
    PacketHeader {
        signature: SIGNATURE,
        size: HEADER_LEN as u32,
        seconds: 1,
        nanoseconds: 2,
        version_major: 0,
        version_minor: 1,
        version_patch: 0,
        packet_type: PACKET_TYPE_HVDIR_LIST,
    }
    .write_to(&mut packet)
    .unwrap();

    let mut buffer = Vec::new();
    {
        let mut writer = QrawWriter::new(&mut buffer).unwrap();
        writer.write_packet(123, &packet).unwrap();
    }

    let mut reader = QrawReader::new(&buffer[..]).unwrap();
    let raw = reader.next_packet().unwrap().unwrap();
    assert_eq!(raw.arrival_delta_ns, 123);
    assert_eq!(raw.bytes, packet);
    assert!(reader.next_packet().unwrap().is_none());
}
