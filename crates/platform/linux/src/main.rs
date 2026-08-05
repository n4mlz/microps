use std::{thread, time::Duration};

use linux::{EtherTapDevice, LinuxPlatform, should_terminate, stack};
use microps::{DeviceKind, DeviceMeta, Irq, IrqLine, Stack, debug, error, protocol::Ipv4Interface};

// These values must match scripts/linux_tap_up.sh:
// Linux host = 10.0.0.1/24, microps = 10.0.0.2/24.
const TAP_NAME: &str = "microps0";
const TAP_IP: [u8; 4] = [10, 0, 0, 2];
const TAP_NETMASK: [u8; 4] = [255, 255, 255, 0];

fn main() {
    Stack::<LinuxPlatform>::init().unwrap();

    let stack = stack();
    let device_key;
    let interface_key;
    {
        device_key = stack.register_device(
            DeviceMeta::new(TAP_NAME, DeviceKind::Ethernet, 1500),
            EtherTapDevice::new(TAP_NAME),
        );
        interface_key = stack
            .interfaces
            .register(Ipv4Interface::new(TAP_IP.into(), TAP_NETMASK.into()));
        stack
            .interfaces
            .attach(interface_key, device_key)
            .expect("interface attaches to TAP device");
    }

    <LinuxPlatform as Irq>::register(
        IrqLine::DeviceInput,
        Box::new(move |_| {
            // Release the device registry lock before raising SoftInput.
            let result = {
                let mut devices = stack
                    .devices
                    .acquire()
                    .expect("device registry lock is infallible");
                devices
                    .get_mut(device_key)
                    .ok_or(microps::StackError::DeviceNotFound)
                    .and_then(|device| device.input().map_err(microps::StackError::Device))
            };
            if let Err(error_value) = result {
                error!("device input failure: {error_value}");
                return;
            }
            if let Err(error_value) = <LinuxPlatform as Irq>::raise(IrqLine::SoftInput) {
                error!("soft input interrupt failure: {error_value:?}");
            }
        }),
    )
    .unwrap();

    <LinuxPlatform as Irq>::register(
        IrqLine::SoftInput,
        Box::new(move |_| {
            // This handler locks only the resources it needs while draining
            // the queue.
            if let Err(error_value) = stack.soft_input() {
                error!("input processing failure: {error_value}");
            }
        }),
    )
    .unwrap();

    if let Err(error_value) = stack.open_all() {
        error!("device initialization failure: {error_value}");
        Stack::<LinuxPlatform>::shutdown();
        return;
    }

    debug!("interface={interface_key:?}, press Ctrl+C to terminate");
    while !should_terminate() {
        thread::sleep(Duration::from_millis(10));
    }

    debug!("terminate");
    stack.close_all();
    Stack::<LinuxPlatform>::shutdown();
}
