use std::{thread, time::Duration};

use linux::{LinuxPlatform, should_terminate};
use microps::{Device, DeviceKind, DeviceMeta, DeviceRegistry, LoopbackDevice, Stack};

const TEST_DATA: &[u8] = &[0x45, 0x00, 0x00, 0x30, 0x00, 0x01, 0x00, 0x00];

fn main() {
    Stack::init::<LinuxPlatform>().unwrap();

    let mut registry = DeviceRegistry::new();
    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Loopback, 65_535),
        LoopbackDevice::new(),
    ));
    registry.open_all().unwrap();

    while !should_terminate() {
        registry
            .device_mut(handle)
            .unwrap()
            .output(0x0800, TEST_DATA, None)
            .unwrap();
        thread::sleep(Duration::from_millis(1));
    }

    registry.close_all();
    Stack::shutdown::<LinuxPlatform>();
}
