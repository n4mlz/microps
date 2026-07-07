use microps::{Device, DeviceKind, DeviceMeta, DeviceRegistry, DummyDevice};

#[test]
fn dummy_device_runs_through_the_registry() {
    let mut registry = DeviceRegistry::new();
    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Dummy, 128),
        DummyDevice::new(),
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
        .expect("output succeeds");
    registry.close_all();
}
