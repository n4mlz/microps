use std::{
    mem,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use linux::{EtherTapDevice, LinuxPlatform, SOFT_IRQ, ether_tap_irq, should_terminate};
use microps::{
    Device, DeviceKind, DeviceMeta, DeviceRegistry, InterfaceRegistry, Irq, ProtocolInputQueue,
    Stack, debug, error, input,
    protocol::{Ipv4Addr, Ipv4Interface},
    soft_input,
};

const TAP_NAME: &str = "microps0";
const TAP_ADDRESS: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
const TAP_IP: [u8; 4] = [10, 0, 0, 2];
const TAP_NETMASK: [u8; 4] = [255, 255, 255, 0];

fn main() {
    Stack::init::<LinuxPlatform>().unwrap();

    let mut runtime = Runtime {
        registry: Mutex::new(DeviceRegistry::new()),
        interfaces: InterfaceRegistry::new(),
        queue: Mutex::new(ProtocolInputQueue::new()),
        handle: 0,
    };
    runtime.handle = runtime
        .registry
        .get_mut()
        .expect("registry mutex is not poisoned")
        .register(Device::new(
            DeviceMeta::new(TAP_NAME, DeviceKind::Ethernet, 1500),
            EtherTapDevice::new(TAP_NAME, TAP_ADDRESS.into()),
        ));
    runtime
        .interfaces
        .add(Ipv4Interface::new(
            Ipv4Addr::from(TAP_IP),
            Ipv4Addr::from(TAP_NETMASK),
        ))
        .expect("TAP interface registers");
    let runtime = Arc::new(runtime);
    let soft_runtime = Arc::clone(&runtime);
    LinuxPlatform::register(
        SOFT_IRQ,
        Box::new(move || {
            let mut queue = {
                let mut queue = soft_runtime.queue.lock().expect("queue mutex poisoned");
                mem::take(&mut *queue)
            };
            soft_input(&mut queue, &soft_runtime.interfaces);
        }),
    )
    .expect("soft IRQ registers");
    let irq_runtime = Arc::clone(&runtime);
    LinuxPlatform::register(
        ether_tap_irq(),
        Box::new(move || input_interrupt(&irq_runtime)),
    )
    .expect("TAP IRQ registers");
    LinuxPlatform::run().expect("IRQ dispatcher starts");

    if let Err(error_value) = runtime
        .registry
        .lock()
        .expect("registry mutex poisoned")
        .open_all()
    {
        error!("device initialization failure: {error_value}");
        Stack::shutdown::<LinuxPlatform>();
        return;
    }

    debug!("press Ctrl+C to terminate");
    while !should_terminate() {
        thread::sleep(Duration::from_millis(10));
    }

    debug!("terminate");
    Stack::shutdown::<LinuxPlatform>();
    let runtime = Arc::try_unwrap(runtime).expect("IRQ runtime still has references");
    runtime
        .registry
        .lock()
        .expect("registry mutex poisoned")
        .close_all();
}

#[derive(Debug)]
struct Runtime {
    registry: Mutex<DeviceRegistry>,
    interfaces: InterfaceRegistry,
    queue: Mutex<ProtocolInputQueue>,
    handle: usize,
}

fn input_interrupt(runtime: &Arc<Runtime>) {
    let result = {
        let mut registry = runtime.registry.lock().expect("registry mutex poisoned");
        let mut queue = runtime.queue.lock().expect("queue mutex poisoned");
        input(&mut registry, runtime.handle, &mut queue)
    };
    if let Err(error_value) = result {
        error!("device input failure: {error_value}");
    } else if let Err(error_value) = LinuxPlatform::raise(SOFT_IRQ) {
        error!("soft IRQ raise failure: {error_value}");
    }
}
