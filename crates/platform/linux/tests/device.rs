use linux::LinuxPlatform;
use microps::{
    Device, DeviceKind, DeviceMeta, DeviceRegistry, InterfaceRegistry, LoopbackDevice, Stack,
    net_input,
    protocol::{Ipv4Addr, Ipv4Interface},
};

const TEST_DATA: &[u8] = &[
    0x45, 0x00, 0x00, 0x30, 0x00, 0x80, 0x00, 0x00, 0xff, 0x01, 0xbd, 0x4a, 0x7f, 0x00, 0x00, 0x01,
    0x7f, 0x00, 0x00, 0x01, 0x08, 0x00, 0x35, 0x64, 0x00, 0x80, 0x00, 0x01, 0x31, 0x32, 0x33, 0x34,
    0x35, 0x36, 0x37, 0x38, 0x39, 0x30, 0x21, 0x40, 0x23, 0x24, 0x25, 0x5e, 0x26, 0x2a, 0x28, 0x29,
];

#[test]
fn loopback_device_runs_through_the_stack() {
    Stack::init::<LinuxPlatform>().expect("stack initializes");

    let mut registry = DeviceRegistry::new();
    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Loopback, 65_535),
        LoopbackDevice::new(),
    ));

    let mut interfaces = InterfaceRegistry::new();
    interfaces
        .add(Ipv4Interface::new(
            Ipv4Addr::from([127, 0, 0, 1]),
            Ipv4Addr::from([255, 0, 0, 0]),
        ))
        .expect("loopback interface registers");

    assert_eq!(
        registry
            .device(handle)
            .expect("device exists")
            .meta()
            .name(),
        "net0"
    );

    registry.open_all().expect("registry opens");
    registry
        .device_mut(handle)
        .expect("device exists")
        .output(0x0800, TEST_DATA, None)
        .expect("loopback output succeeds");
    net_input(
        registry.device(handle).expect("device exists").meta(),
        &interfaces,
        0x0800,
        TEST_DATA,
    );
    registry.close_all();
    Stack::shutdown::<LinuxPlatform>();
}
