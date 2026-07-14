use std::{thread, time::Duration};

use linux::{LinuxPlatform, should_terminate};
use microps::{
    Device, DeviceKind, DeviceMeta, DeviceRegistry, LoopbackDevice, Stack, debug, error,
};

const TEST_DATA: &[u8] = &[
    0x45, 0x00, 0x00, 0x30, 0x00, 0x80, 0x00, 0x00, 0xff, 0x01, 0xbd, 0x4a, 0x7f, 0x00, 0x00, 0x01,
    0x7f, 0x00, 0x00, 0x01, 0x08, 0x00, 0x35, 0x64, 0x00, 0x80, 0x00, 0x01, 0x31, 0x32, 0x33, 0x34,
    0x35, 0x36, 0x37, 0x38, 0x39, 0x30, 0x21, 0x40, 0x23, 0x24, 0x25, 0x5e, 0x26, 0x2a, 0x28, 0x29,
];

fn main() {
    Stack::init::<LinuxPlatform>().unwrap();

    let mut registry = DeviceRegistry::new();
    let handle = registry.register(Device::new(
        DeviceMeta::new("net0", DeviceKind::Loopback, 65_535),
        LoopbackDevice::new(),
    ));
    registry.open_all().unwrap();

    debug!("press Ctrl+C to terminate");
    while !should_terminate() {
        let result = registry
            .device_mut(handle)
            .unwrap()
            .output(0x0800, TEST_DATA, None);
        if let Err(error_value) = result {
            error!("net_device_output() failure: {error_value}");
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    debug!("terminate");
    registry.close_all();
    Stack::shutdown::<LinuxPlatform>();
}
