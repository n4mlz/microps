use std::{thread, time::Duration};

use linux::{EtherTapDevice, LinuxPlatform, should_terminate, stack};
use microps::{
    DeviceKind, DeviceMeta, Irq, IrqLine, Stack, debug, error,
    protocol::{Ipv4Addr, Ipv4Endpoint, Ipv4Interface, Tcp, TcpOpenMode},
};

// These values must match scripts/linux_tap_up.sh:
// Linux host = 10.0.0.1/24, microps = 10.0.0.2/24.
const TAP_NAME: &str = "microps0";
const TAP_MAC: [u8; 6] = [0x00, 0x00, 0x5e, 0x00, 0x53, 0x01];
const TAP_IP: [u8; 4] = [10, 0, 0, 2];
const TAP_NETMASK: [u8; 4] = [255, 255, 255, 0];
const TCP_REMOTE: [u8; 4] = [10, 0, 0, 1];
const TCP_REMOTE_PORT: u16 = 10007;

fn main() {
    Stack::<LinuxPlatform>::init().unwrap();

    let stack = stack();
    let device_key;
    let interface_key;
    {
        device_key = stack.devices.register_device(
            DeviceMeta::new(TAP_NAME, DeviceKind::Ethernet, 1500),
            EtherTapDevice::new(TAP_NAME, TAP_MAC.into()),
        );
        interface_key = stack
            .interfaces
            .register(Ipv4Interface::new(TAP_IP.into(), TAP_NETMASK.into()));
        stack
            .interfaces
            .attach(interface_key, device_key)
            .expect("interface attaches to TAP device");
        stack
            .ipv4_routes
            .set_default_gateway(interface_key, [10, 0, 0, 1].into());
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

    let tcp_pcb = match Tcp::open::<LinuxPlatform>(
        Ipv4Endpoint::new(Ipv4Addr::ANY, 0),
        Ipv4Endpoint::new(TCP_REMOTE.into(), TCP_REMOTE_PORT),
        TcpOpenMode::Active,
    ) {
        Ok(pcb) => pcb,
        Err(error_value) => {
            error!("TCP open failure: {error_value}");
            stack.close_all();
            Stack::<LinuxPlatform>::shutdown();
            return;
        }
    };

    debug!("interface={interface_key:?}, TCP connection established");
    let echo_thread = thread::spawn(move || {
        let mut buffer = [0; 128];
        loop {
            match Tcp::receive::<LinuxPlatform>(tcp_pcb, &mut buffer) {
                Ok(length) => {
                    debug!("received {length} bytes");
                    microps::debugdump(&buffer[..length]);
                    if let Err(error_value) = Tcp::send::<LinuxPlatform>(tcp_pcb, &buffer[..length])
                    {
                        error!("TCP send failure: {error_value}");
                        break;
                    }
                }
                Err(error_value) => {
                    if !should_terminate() {
                        error!("TCP receive failure: {error_value}");
                    }
                    break;
                }
            }
        }
    });
    while !should_terminate() {
        if let Err(error_value) = Tcp::tick::<LinuxPlatform>() {
            error!("TCP retrans failure: {error_value}");
        }
        thread::sleep(Duration::from_millis(100));
    }

    debug!("terminate");
    if let Err(error_value) = Tcp::close::<LinuxPlatform>(tcp_pcb) {
        error!("TCP close failure: {error_value}");
    }
    let _ = echo_thread.join();
    stack.close_all();
    Stack::<LinuxPlatform>::shutdown();
}
