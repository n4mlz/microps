use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use linux::{EtherTapDevice, LinuxPlatform, ether_tap_irq, should_terminate};
use microps::{DeviceKind, DeviceMeta, Irq, IrqLine, Stack, debug, error, protocol::Ipv4Interface};

// These values must match scripts/linux_tap_up.sh:
// Linux host = 10.0.0.1/24, microps = 10.0.0.2/24.
const TAP_NAME: &str = "microps0";
const TAP_IP: [u8; 4] = [10, 0, 0, 2];
const TAP_NETMASK: [u8; 4] = [255, 255, 255, 0];

fn main() {
    Stack::<LinuxPlatform>::init().unwrap();

    let stack = Arc::new(Mutex::new(Stack::<LinuxPlatform>::new()));
    let device_key;
    let interface_key;
    {
        let stack = &mut *stack.lock().expect("stack mutex poisoned");
        let input_queue = stack.input_queue().clone();
        device_key = stack.register_device(
            DeviceMeta::new(TAP_NAME, DeviceKind::Ethernet, 1500),
            EtherTapDevice::new(TAP_NAME, input_queue),
        );
        interface_key = stack
            .interfaces
            .register(Ipv4Interface::new(TAP_IP.into(), TAP_NETMASK.into()));
        stack
            .interfaces
            .attach(interface_key, device_key)
            .expect("interface attaches to TAP device");
    }

    let receive_stack = Arc::clone(&stack);
    <LinuxPlatform as Irq>::register(
        ether_tap_irq(),
        Box::new(move |_| {
            let mut stack = receive_stack.lock().expect("stack mutex poisoned");
            let result = stack
                .devices
                .device_mut(device_key)
                .ok_or(microps::StackError::DeviceNotFound)
                .and_then(|device| device.input().map_err(microps::StackError::Device));
            if let Err(error_value) = result {
                error!("device input failure: {error_value}");
            }
        }),
    )
    .unwrap();

    let input_stack = Arc::clone(&stack);
    <LinuxPlatform as Irq>::register(
        IrqLine::SoftInput,
        Box::new(move |_| {
            if let Err(error_value) = input_stack
                .lock()
                .expect("stack mutex poisoned")
                .soft_input()
            {
                error!("input processing failure: {error_value}");
            }
        }),
    )
    .unwrap();

    if let Err(error_value) = stack.lock().expect("stack mutex poisoned").open_all() {
        error!("device initialization failure: {error_value}");
        Stack::<LinuxPlatform>::shutdown();
        return;
    }

    debug!("interface={interface_key:?}, press Ctrl+C to terminate");
    while !should_terminate() {
        thread::sleep(Duration::from_millis(10));
    }

    debug!("terminate");
    stack.lock().expect("stack mutex poisoned").close_all();
    Stack::<LinuxPlatform>::shutdown();
}
