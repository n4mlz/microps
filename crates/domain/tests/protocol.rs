use microps::protocol::{
    ArpError, ArpOperation, ArpPacket, EtherType, EthernetError, EthernetFrame, IcmpError,
    IcmpHeader, IcmpPacket, IcmpType, Ipv4Addr, Ipv4Endpoint, Ipv4Error, Ipv4Header, Ipv4Interface,
    Ipv4Packet, Ipv4Protocol, MacAddr, TcpError, TcpFlags, TcpPacket, UdpError, UdpPacket,
};

#[test]
fn ethernet_ipv4_type_matches_the_raw_value() {
    assert_eq!(EtherType::Ipv4 as u16, 0x0800);
}

#[test]
fn mac_address_parses_and_formats_colon_hex() {
    let address = "02:00:5e:10:20:30"
        .parse::<MacAddr>()
        .expect("valid MAC address");

    assert_eq!(address.bytes(), [2, 0, 0x5e, 0x10, 0x20, 0x30]);
    assert_eq!(address.to_string(), "02:00:5e:10:20:30");
}

#[test]
fn ethernet_frame_round_trips_header_and_payload() {
    let src = MacAddr::from([2, 0, 0, 0, 0, 1]);
    let dest = MacAddr::BROADCAST;
    let bytes =
        EthernetFrame::build(src, dest, EtherType::Ipv4, &[0xaa, 0xbb]).expect("frame builds");
    let frame = EthernetFrame::try_from(&bytes[..]).expect("frame parses");

    assert_eq!(frame.header().src(), src);
    assert_eq!(frame.header().dest(), dest);
    assert_eq!(frame.header().ether_type(), EtherType::Ipv4 as u16);
    assert_eq!(frame.payload(), &[0xaa, 0xbb]);
}

#[test]
fn ethernet_frame_rejects_short_frames_and_large_payloads() {
    assert_eq!(
        EthernetFrame::try_from(&[0; 13][..]),
        Err(EthernetError::TooShort { len: 13 })
    );
    assert_eq!(
        EthernetFrame::build(MacAddr::ANY, MacAddr::ANY, EtherType::Ipv4, &[0; 1501]),
        Err(EthernetError::PayloadTooLarge { len: 1501 })
    );
}

#[test]
fn ipv4_addr_parses_and_formats_dotted_decimal() {
    let addr = "192.0.2.1".parse::<Ipv4Addr>().expect("valid address");

    assert_eq!(addr.as_bytes(), &[192, 0, 2, 1]);
    assert_eq!(addr.to_string(), "192.0.2.1");
    assert_eq!(Ipv4Addr::ANY.to_string(), "0.0.0.0");
    assert_eq!(Ipv4Addr::BROADCAST.to_string(), "255.255.255.255");
}

#[test]
fn ipv4_addr_rejects_invalid_dotted_decimal() {
    assert!("192.0.2".parse::<Ipv4Addr>().is_err());
    assert!("192.0.2.256".parse::<Ipv4Addr>().is_err());
    assert!("192.0.2.1x".parse::<Ipv4Addr>().is_err());
}

#[test]
fn ipv4_endpoint_parses_and_formats() {
    let endpoint = "192.0.2.1:5353"
        .parse::<Ipv4Endpoint>()
        .expect("valid endpoint");

    assert_eq!(endpoint.address(), Ipv4Addr::from([192, 0, 2, 1]));
    assert_eq!(endpoint.port(), 5353);
    assert_eq!(endpoint.to_string(), "192.0.2.1:5353");
}

#[test]
fn ipv4_endpoint_rejects_invalid_values() {
    assert!("192.0.2.1".parse::<Ipv4Endpoint>().is_err());
    assert!("192.0.2.1:65536".parse::<Ipv4Endpoint>().is_err());
    assert!("192.0.2.256:53".parse::<Ipv4Endpoint>().is_err());
}

#[test]
fn ipv4_interface_calculates_broadcast_and_accepts_local_destinations() {
    let interface = Ipv4Interface::new(
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([255, 255, 255, 0]),
    );

    assert_eq!(interface.unicast(), Ipv4Addr::from([192, 0, 2, 1]));
    assert_eq!(interface.netmask(), Ipv4Addr::from([255, 255, 255, 0]));
    assert_eq!(interface.broadcast(), Ipv4Addr::from([192, 0, 2, 255]));
    assert!(interface.accepts(&[192, 0, 2, 1]));
    assert!(interface.accepts(&[192, 0, 2, 255]));
    assert!(interface.accepts(&[255, 255, 255, 255]));
    assert!(!interface.accepts(&[192, 0, 3, 1]));
}

#[test]
fn ipv4_packet_parses_a_valid_header() {
    let header = Ipv4Header::try_from(
        &[
            0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x11, 0x7c, 0x6e, 0xc0, 0x00,
            0x02, 0x01, 0xc6, 0x33, 0x64, 0x02,
        ][..],
    )
    .expect("valid IPv4 header");

    assert_eq!(header.version(), 4);
    assert_eq!(header.id(), 0x1234);
    assert_eq!(header.flags(), 0);
    assert_eq!(header.fragment_offset(), 0);
    assert_eq!(header.ttl(), 64);
    assert_eq!(header.protocol(), 17);
    assert_eq!(header.checksum(), Some(0x7c6e));
    assert_eq!(header.src(), Ipv4Addr::from([192, 0, 2, 1]));
    assert_eq!(header.dest(), Ipv4Addr::from([198, 51, 100, 2]));

    let bytes = header.to_bytes(20);
    assert_eq!(bytes, valid_ipv4_header());

    let bytes = valid_ipv4_header();
    let packet = Ipv4Packet::try_from(&bytes[..]).expect("valid IPv4 packet");
    assert_eq!(packet.header(), header);
    assert_eq!(packet.payload(), &[]);
}

#[test]
fn ipv4_packet_keeps_header_and_payload_separate() {
    let mut bytes = valid_ipv4_header();
    bytes[2..4].copy_from_slice(&22u16.to_be_bytes());
    bytes[10..12].copy_from_slice(&0x7c6cu16.to_be_bytes());
    let mut packet_bytes = bytes.to_vec();
    packet_bytes.extend_from_slice(&[0xaa, 0xbb]);

    let packet = Ipv4Packet::try_from(&packet_bytes[..]).expect("valid IPv4 packet");

    assert_eq!(packet.packet_len(), 22);
    assert_eq!(packet.payload(), &[0xaa, 0xbb]);
}

#[test]
fn ipv4_packet_builds_a_valid_header_and_payload() {
    let packet = Ipv4Packet::build(
        1,
        &[0xaa, 0xbb],
        0x1234,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    )
    .expect("packet builds");

    let parsed = Ipv4Packet::try_from(&packet[..]).expect("built packet parses");
    assert_eq!(parsed.header().id(), 0x1234);
    assert_eq!(parsed.header().ttl(), 255);
    assert_eq!(parsed.header().protocol(), 1);
    assert_eq!(parsed.payload(), &[0xaa, 0xbb]);
}

#[test]
fn ipv4_protocol_numbers_are_typed() {
    assert_eq!(Ipv4Protocol::try_from(1), Ok(Ipv4Protocol::Icmp));
    assert_eq!(Ipv4Protocol::try_from(6), Ok(Ipv4Protocol::Tcp));
    assert_eq!(Ipv4Protocol::try_from(17), Ok(Ipv4Protocol::Udp));
    assert!(Ipv4Protocol::try_from(99).is_err());
}

#[test]
fn udp_packet_parses_declared_length_and_ports() {
    let udp = [
        0x04, 0xd2, 0x16, 0x2e, 0x00, 0x0b, 0x00, 0x00, b'h', b'i', b'!',
    ];
    let ipv4 = Ipv4Packet::build(
        Ipv4Protocol::Udp as u8,
        &udp,
        0,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    )
    .expect("packet builds");

    let packet =
        UdpPacket::from_ipv4(Ipv4Packet::try_from(&ipv4[..]).unwrap()).expect("UDP packet parses");
    assert_eq!(packet.src().to_string(), "192.0.2.1:1234");
    assert_eq!(packet.dest().to_string(), "192.0.2.2:5678");
    assert_eq!(packet.payload(), b"hi!");
}

#[test]
fn udp_packet_rejects_invalid_lengths_and_checksums() {
    let mut udp = [0; 8];
    udp[4..6].copy_from_slice(&7u16.to_be_bytes());
    let ipv4 = Ipv4Packet::build(
        Ipv4Protocol::Udp as u8,
        &udp,
        0,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    )
    .unwrap();
    assert_eq!(
        UdpPacket::from_ipv4(Ipv4Packet::try_from(&ipv4[..]).unwrap()),
        Err(UdpError::LengthTooSmall { length: 7 })
    );

    udp[4..6].copy_from_slice(&8u16.to_be_bytes());
    udp[6..8].copy_from_slice(&1u16.to_be_bytes());
    let ipv4 = Ipv4Packet::build(
        Ipv4Protocol::Udp as u8,
        &udp,
        0,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    )
    .unwrap();
    assert_eq!(
        UdpPacket::from_ipv4(Ipv4Packet::try_from(&ipv4[..]).unwrap()),
        Err(UdpError::InvalidChecksum)
    );
}

#[test]
fn tcp_packet_parses_header_options_and_payload() {
    let tcp = [
        0x04, 0xd2, 0x00, 0x50, 0x01, 0x02, 0x03, 0x04, 0, 0, 0, 0, 0x60, 0x02, 0xff, 0xff, 0xa2,
        0x8f, 0, 0, 0x02, 0x04, 0x05, 0xb4, b'h', b'i',
    ];
    let ipv4 = Ipv4Packet::build(
        Ipv4Protocol::Tcp as u8,
        &tcp,
        0,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    )
    .expect("packet builds");

    let packet =
        TcpPacket::from_ipv4(Ipv4Packet::try_from(&ipv4[..]).unwrap()).expect("TCP packet parses");
    assert_eq!(packet.src().to_string(), "192.0.2.1:1234");
    assert_eq!(packet.dest().to_string(), "192.0.2.2:80");
    assert_eq!(packet.header().sequence_number(), 0x0102_0304);
    assert_eq!(packet.header().header_len(), 24);
    assert_eq!(packet.header().flags(), TcpFlags::SYN);
    assert_eq!(packet.options(), &[2, 4, 5, 180]);
    assert_eq!(packet.payload(), b"hi");
}

#[test]
fn tcp_packet_rejects_short_headers_and_bad_checksums() {
    let ipv4 = Ipv4Packet::build(
        Ipv4Protocol::Tcp as u8,
        &[0; 19],
        0,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    )
    .unwrap();
    assert_eq!(
        TcpPacket::from_ipv4(Ipv4Packet::try_from(&ipv4[..]).unwrap()),
        Err(TcpError::TooShort { len: 19 })
    );

    let mut tcp = [0; 20];
    tcp[12] = 0x50;
    tcp[16..18].copy_from_slice(&1u16.to_be_bytes());
    let ipv4 = Ipv4Packet::build(
        Ipv4Protocol::Tcp as u8,
        &tcp,
        0,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    )
    .unwrap();
    assert_eq!(
        TcpPacket::from_ipv4(Ipv4Packet::try_from(&ipv4[..]).unwrap()),
        Err(TcpError::InvalidChecksum)
    );
}

#[test]
fn icmp_type_numbers_are_typed() {
    assert_eq!(IcmpType::Echo as u8, 8);
    assert_eq!(IcmpType::try_from(8), Ok(IcmpType::Echo));
    assert!(IcmpType::try_from(99).is_err());
}

#[test]
fn icmp_packet_preserves_ipv4_addresses_and_payload() {
    let header = Ipv4Header::new(
        Ipv4Protocol::Icmp as u8,
        0,
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([192, 0, 2, 2]),
    );
    let ipv4_packet = Ipv4Packet::build(
        Ipv4Protocol::Icmp as u8,
        &[0x08, 0x00, 0x4d, 0x42, 0x00, 0x01, 0x00, 0x01, 0xaa, 0xbb],
        0,
        header.src(),
        header.dest(),
    )
    .expect("packet builds");
    let packet =
        IcmpPacket::from_ipv4(Ipv4Packet::try_from(&ipv4_packet[..]).expect("packet parses"))
            .expect("ICMP packet parses");

    assert_eq!(packet.src(), Ipv4Addr::from([192, 0, 2, 1]));
    assert_eq!(packet.dest(), Ipv4Addr::from([192, 0, 2, 2]));
    assert_eq!(packet.payload(), &[0xaa, 0xbb]);
}

#[test]
fn icmp_header_rejects_short_and_corrupt_messages() {
    assert!(IcmpHeader::try_from(&[0; 7][..]).is_err());
    assert_eq!(
        IcmpHeader::try_from(&[0x08, 0x00, 0, 0, 0, 1, 0, 1][..]),
        Err(IcmpError::InvalidChecksum)
    );
}

#[test]
fn arp_packet_round_trips_ethernet_ipv4_fields() {
    let packet = ArpPacket::build(
        ArpOperation::Request,
        MacAddr::from([2, 0, 0, 0, 0, 1]),
        Ipv4Addr::from([192, 0, 2, 1]),
        MacAddr::ANY,
        Ipv4Addr::from([192, 0, 2, 2]),
    );
    let packet = ArpPacket::try_from(&packet[..]).expect("ARP packet parses");

    assert_eq!(packet.header().hardware_type(), 1);
    assert_eq!(packet.header().protocol_type(), EtherType::Ipv4 as u16);
    assert_eq!(packet.header().hardware_len(), 6);
    assert_eq!(packet.header().protocol_len(), 4);
    assert_eq!(packet.header().operation(), ArpOperation::Request as u16);
    assert_eq!(packet.sender_hardware(), MacAddr::from([2, 0, 0, 0, 0, 1]));
    assert_eq!(packet.sender_protocol(), Ipv4Addr::from([192, 0, 2, 1]));
    assert_eq!(packet.target_hardware(), MacAddr::ANY);
    assert_eq!(packet.target_protocol(), Ipv4Addr::from([192, 0, 2, 2]));
}

#[test]
fn arp_packet_rejects_unsupported_address_formats() {
    let mut packet = ArpPacket::build(
        ArpOperation::Request,
        MacAddr::ANY,
        Ipv4Addr::ANY,
        MacAddr::ANY,
        Ipv4Addr::ANY,
    );
    packet[4] = 5;
    assert_eq!(
        ArpPacket::try_from(&packet[..]),
        Err(ArpError::UnsupportedHardware {
            hardware_type: 1,
            hardware_len: 5,
        })
    );

    packet[4] = 6;
    packet[2..4].copy_from_slice(&0x86ddu16.to_be_bytes());
    assert_eq!(
        ArpPacket::try_from(&packet[..]),
        Err(ArpError::UnsupportedProtocol {
            protocol_type: 0x86dd,
            protocol_len: 4,
        })
    );
}

#[test]
fn ipv4_packet_rejects_invalid_inputs() {
    assert_eq!(
        Ipv4Packet::try_from(&[0; 19][..]),
        Err(Ipv4Error::TooShort { len: 19 })
    );

    let mut wrong_version = valid_ipv4_header();
    wrong_version[0] = 0x65;
    assert_eq!(
        Ipv4Header::try_from(&wrong_version[..]),
        Err(Ipv4Error::InvalidVersion { version: 6 })
    );

    let mut fragmented = valid_ipv4_header();
    fragmented[6] = 0x20;
    fragmented[10] = 0x5c;
    fragmented[11] = 0x6e;
    assert_eq!(
        Ipv4Packet::try_from(&fragmented[..]),
        Err(Ipv4Error::Fragmented)
    );
}

fn valid_ipv4_header() -> [u8; 20] {
    [
        0x45, 0x00, 0x00, 0x14, 0x12, 0x34, 0x00, 0x00, 0x40, 0x11, 0x7c, 0x6e, 0xc0, 0x00, 0x02,
        0x01, 0xc6, 0x33, 0x64, 0x02,
    ]
}
