use linux::LinuxPlatform;
use microps::{
    DeviceKind, DeviceMeta, Irq, IrqLine, LoopbackDevice, Stack,
    protocol::{Ipv4Addr, Ipv4Interface},
};

const TEST_DATA: &[u8] = &[
    0x45, 0x00, 0x00, 0x30, 0x00, 0x80, 0x00, 0x00, 0xff, 0x01, 0xbd, 0x4a, 0x7f, 0x00, 0x00, 0x01,
    0x7f, 0x00, 0x00, 0x01, 0x08, 0x00, 0x35, 0x64, 0x00, 0x80, 0x00, 0x01, 0x31, 0x32, 0x33, 0x34,
    0x35, 0x36, 0x37, 0x38, 0x39, 0x30, 0x21, 0x40, 0x23, 0x24, 0x25, 0x5e, 0x26, 0x2a, 0x28, 0x29,
];

#[test]
fn loopback_device_runs_through_the_stack() {
    Stack::<LinuxPlatform>::init().expect("stack initializes");
    <LinuxPlatform as Irq>::register(IrqLine::SoftInput, |_line, _arg| {}, 0)
        .expect("soft IRQ registers");

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

    stack.open_all().expect("stack opens devices");
    let source = Ipv4Addr::from([127, 0, 0, 1]);
    let (interfaces, devices) = (&mut stack.interfaces, &mut stack.devices);
    interfaces
        .interface_as::<Ipv4Interface>(interface_key)
        .expect("interface exists")
        .output::<LinuxPlatform, LinuxPlatform>(devices, 1, &TEST_DATA[20..], source, source)
        .expect("loopback output succeeds");
    stack.close_all();
    Stack::<LinuxPlatform>::shutdown();
}
