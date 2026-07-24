use linux::LinuxPlatform;
use microps::{Device, DeviceKind, DeviceMeta, DeviceRegistry, LoopbackDevice, Stack};

#[test]
fn loopback_device_runs_through_the_stack() {
    Stack::init::<LinuxPlatform>().expect("stack initializes");

    let mut registry = DeviceRegistry::new();
    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Loopback, 65_535),
        LoopbackDevice::new(),
    ));

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
        .output(0x0800, &[0x45, 0x00, 0x00, 0x30], None)
        .expect("loopback output succeeds");
    registry.close_all();
    Stack::shutdown::<LinuxPlatform>();
}
