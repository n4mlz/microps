use microps::protocol::{
    EthernetAddress, EthernetError, EthernetFrame, Ipv4, Ipv4Addr, Ipv4Error, Ipv4Header,
    Ipv4Interface, Ipv4Packet, Protocol, ethertype,
};

#[test]
fn ethernet_common_ethertypes_match_wire_values() {
    assert_eq!(ethertype::IPV4, 0x0800);
    assert_eq!(ethertype::ARP, 0x0806);
    assert_eq!(ethertype::IPV6, 0x86dd);
}

#[test]
fn ethernet_address_parses_and_formats_colon_hex() {
    let address = "02:00:5e:10:20:ff"
        .parse::<EthernetAddress>()
        .expect("valid Ethernet address");

    assert_eq!(address.octets(), [0x02, 0x00, 0x5e, 0x10, 0x20, 0xff]);
    assert_eq!(address.to_string(), "02:00:5e:10:20:ff");
    assert_eq!(EthernetAddress::ANY.to_string(), "00:00:00:00:00:00");
    assert_eq!(EthernetAddress::BROADCAST.to_string(), "ff:ff:ff:ff:ff:ff");
}

#[test]
fn ethernet_address_rejects_malformed_text() {
    assert!("02:00:5e:10:20".parse::<EthernetAddress>().is_err());
    assert!("02:00:5e:10:20:100".parse::<EthernetAddress>().is_err());
    assert!("02-00-5e-10-20-ff".parse::<EthernetAddress>().is_err());
}

#[test]
fn ethernet_frame_parses_header_and_borrows_payload() {
    let frame = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02, 0x00, 0x5e, 0x10, 0x20, 0xff, 0x08, 0x00, 0xaa,
        0xbb,
    ];

    let frame = EthernetFrame::try_from(&frame[..]).expect("valid Ethernet frame");

    assert_eq!(frame.destination(), EthernetAddress::BROADCAST);
    assert_eq!(
        frame.source(),
        EthernetAddress::from([0x02, 0x00, 0x5e, 0x10, 0x20, 0xff])
    );
    assert_eq!(frame.ethertype(), 0x0800);
    assert_eq!(frame.payload(), &[0xaa, 0xbb]);
}

#[test]
fn ethernet_frame_serializes_header_and_payload() {
    let frame = EthernetFrame::new(
        EthernetAddress::BROADCAST,
        EthernetAddress::from([0x02, 0x00, 0x5e, 0x10, 0x20, 0xff]),
        0x0800,
        &[0xaa, 0xbb],
    );

    let bytes: Vec<u8> = frame.into();
    assert_eq!(
        bytes,
        [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02, 0x00, 0x5e, 0x10, 0x20, 0xff, 0x08, 0x00,
            0xaa, 0xbb,
        ]
    );
}

#[test]
fn ethernet_frame_rejects_short_header() {
    assert_eq!(
        EthernetFrame::try_from(&[0; 13][..]),
        Err(EthernetError::TooShort { len: 13 })
    );
}

#[test]
fn ipv4_protocol_type_matches_the_raw_value() {
    assert_eq!(Ipv4::TYPE, 0x0800);
}

#[test]
fn ipv4_addr_parses_and_formats_dotted_decimal() {
    let addr = "192.0.2.1".parse::<Ipv4Addr>().expect("valid address");

    assert_eq!(addr.octets(), [192, 0, 2, 1]);
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
fn ipv4_interface_exposes_addresses_and_accepts_local_destinations() {
    let interface = Ipv4Interface::new(
        Ipv4Addr::from([192, 0, 2, 1]),
        Ipv4Addr::from([255, 255, 255, 0]),
    );

    assert_eq!(interface.unicast(), Ipv4Addr::from([192, 0, 2, 1]));
    assert_eq!(interface.netmask(), Ipv4Addr::from([255, 255, 255, 0]));
    assert_eq!(interface.broadcast(), Ipv4Addr::from([192, 0, 2, 255]));

    assert!(interface.accepts(&[192, 0, 2, 1]));
    assert!(interface.accepts(&[192, 0, 2, 255]));
    assert!(interface.accepts(&Ipv4Addr::BROADCAST.octets()));
    assert!(!interface.accepts(&[192, 0, 3, 1]));
    assert!(!interface.accepts(&[192, 0, 2]));
    assert!(!interface.accepts(&[192, 0, 2, 1, 0]));
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
    assert_eq!(header.checksum(), 0x7c6e);
    assert_eq!(header.source(), Ipv4Addr::from([192, 0, 2, 1]));
    assert_eq!(header.destination(), Ipv4Addr::from([198, 51, 100, 2]));

    let bytes: [u8; 20] = header.into();
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

    assert_eq!(packet.total_len(), 22);
    assert_eq!(packet.payload(), &[0xaa, 0xbb]);
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
