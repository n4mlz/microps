use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use linux::{EtherTapDevice, LinuxPlatform, ether_tap_irq, should_terminate};
use microps::{
    Device, DeviceKind, DeviceMeta, DeviceRegistry, InterfaceRegistry, Irq, Stack, debug, error,
    input,
    protocol::{Ipv4Addr, Ipv4Interface},
};

const TAP_NAME: &str = "microps0";
const TAP_ADDRESS: [u8; 6] = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
const TAP_IP: [u8; 4] = [10, 0, 0, 2];
const TAP_NETMASK: [u8; 4] = [255, 255, 255, 0];

fn main() {
    Stack::init::<LinuxPlatform>().unwrap();

    let mut runtime = Runtime {
        registry: DeviceRegistry::new(),
        interfaces: InterfaceRegistry::new(),
    };
    runtime.registry.register(Device::new(
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
    let runtime = Arc::new(Mutex::new(runtime));
    let irq_runtime = Arc::clone(&runtime);
    LinuxPlatform::register(
        ether_tap_irq(),
        Box::new(move || input_interrupt(&irq_runtime)),
    )
    .expect("TAP IRQ registers");
    LinuxPlatform::run().expect("IRQ dispatcher starts");

    if let Err(error_value) = runtime
        .lock()
        .expect("runtime mutex poisoned")
        .registry
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
    let mut runtime = Arc::try_unwrap(runtime)
        .expect("IRQ runtime still has references")
        .into_inner()
        .expect("runtime mutex poisoned");
    runtime.registry.close_all();
}

#[derive(Debug)]
struct Runtime {
    registry: DeviceRegistry,
    interfaces: InterfaceRegistry,
}

fn input_interrupt(runtime: &Arc<Mutex<Runtime>>) {
    let result = {
        let mut runtime = runtime.lock().expect("runtime mutex poisoned");
        let Runtime {
            registry,
            interfaces,
        } = &mut *runtime;
        input(registry, interfaces)
    };
    if let Err(error_value) = result {
        error!("device input failure: {error_value}");
    }
}
