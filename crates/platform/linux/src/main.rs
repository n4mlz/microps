use std::{thread, time::Duration};

use linux::{LinuxPlatform, should_terminate};
use microps::{
    DeviceKind, DeviceMeta, LoopbackDevice, Stack, debug, error,
    protocol::{Ipv4Addr, Ipv4Interface},
};

const TEST_DATA: &[u8] = &[
    0x45, 0x00, 0x00, 0x30, 0x00, 0x80, 0x00, 0x00, 0xff, 0x01, 0xbd, 0x4a, 0x7f, 0x00, 0x00, 0x01,
    0x7f, 0x00, 0x00, 0x01, 0x08, 0x00, 0x35, 0x64, 0x00, 0x80, 0x00, 0x01, 0x31, 0x32, 0x33, 0x34,
    0x35, 0x36, 0x37, 0x38, 0x39, 0x30, 0x21, 0x40, 0x23, 0x24, 0x25, 0x5e, 0x26, 0x2a, 0x28, 0x29,
];

fn main() {
    Stack::<LinuxPlatform>::init().unwrap();

    let mut stack = Stack::<LinuxPlatform>::new();
    let device_key = stack.register_device(
        DeviceMeta::new("net0", DeviceKind::Loopback, 65_535),
        LoopbackDevice::new(stack.input_queue().clone()),
    );
    let interface_key = stack.interfaces.register(Ipv4Interface::new(
        Ipv4Addr::from([127, 0, 0, 1]),
        Ipv4Addr::from([255, 0, 0, 0]),
    ));
    stack
        .interfaces
        .attach(interface_key, device_key)
        .expect("interface attaches to loopback device");
    stack.open_all().unwrap();

    let source = Ipv4Addr::from([127, 0, 0, 1]);

    debug!("press Ctrl+C to terminate");
    while !should_terminate() {
        let (interfaces, devices) = (&mut stack.interfaces, &mut stack.devices);
        let result = interfaces
            .interface_as::<Ipv4Interface>(interface_key)
            .unwrap()
            .output::<LinuxPlatform, LinuxPlatform>(devices, 1, &TEST_DATA[20..], source, source);
        if let Err(error_value) = result {
            error!("net_device_output() failure: {error_value}");
            break;
        }
        if let Err(error_value) = stack.process_input() {
            error!("input processing failure: {error_value}");
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    debug!("terminate");
    stack.close_all();
    Stack::<LinuxPlatform>::shutdown();
}
