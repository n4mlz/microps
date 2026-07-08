use microps::{
    DeviceKind, DeviceMeta,
    protocol::{Ipv4, Protocol, Protocols},
};

#[test]
fn ipv4_protocol_type_matches_the_raw_value() {
    assert_eq!(Ipv4::TYPE, 0x0800);
}

#[test]
fn protocols_input_routes_ipv4_frames() {
    let meta = DeviceMeta::new("net0", DeviceKind::Loopback, 65_535);

    assert!(Protocols::input(
        0x0800,
        &meta,
        &[0x45, 0x00, 0x00, 0x30],
        None
    ));
}

#[test]
fn protocols_input_ignores_unknown_values() {
    let meta = DeviceMeta::new("net0", DeviceKind::Loopback, 65_535);

    assert!(!Protocols::input(0x1234, &meta, &[0x00], None));
}
