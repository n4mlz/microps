use thiserror::Error;

use crate::{
    AddressFamily, DeviceError, DeviceRegistry, InputQueue, InterfaceError, InterfaceRegistry,
    Platform, debug, debugdump, error, info,
    protocol::{self, ArpCache, Ipv4RoutingTable},
};

/// Network stack state and ownership root for devices and interfaces.
#[derive(Default)]
pub struct Stack<P: Platform> {
    pub devices: DeviceRegistry<P>,
    pub interfaces: InterfaceRegistry<P>,
    pub input_queue: InputQueue<P>,
    pub arp_cache: ArpCache<P>,
    pub ipv4_routes: Ipv4RoutingTable<P>,
}

#[derive(Debug, Error)]
pub enum StackError {
    #[error("device does not exist")]
    DeviceNotFound,
    #[error("interface does not exist")]
    InterfaceNotFound,
    #[error("device operation failed: {0}")]
    Device(#[from] DeviceError),
    #[error("interface operation failed: {0}")]
    Interface(#[from] InterfaceError),
}

impl<P: Platform + 'static> Stack<P> {
    pub fn new() -> Self {
        Self {
            devices: DeviceRegistry::default(),
            interfaces: InterfaceRegistry::default(),
            input_queue: alloc::sync::Arc::default(),
            arp_cache: ArpCache::new(),
            ipv4_routes: Ipv4RoutingTable::default(),
        }
    }

    pub fn open_all(&self) -> Result<(), StackError> {
        self.devices.open_all()?;
        Ok(())
    }

    pub fn close_all(&self) {
        self.devices.close_all();
    }

    pub fn soft_input(&self) -> Result<(), StackError> {
        while let Some(frame) = self.input_queue.pop() {
            let device = frame.device();
            if !self.devices.contains(device) {
                return Err(StackError::DeviceNotFound);
            }
            debug!(
                "device={device:?}, type=0x{:04x}, len={}",
                frame.frame_type(),
                frame.data().len()
            );
            debugdump(frame.data());
            let family = match frame.frame_type() {
                type_value if type_value == protocol::EtherType::Ipv4 as u16 => AddressFamily::Ipv4,
                type_value if type_value == protocol::EtherType::Arp as u16 => AddressFamily::Ipv4,
                _ => {
                    continue;
                }
            };
            let mut interfaces = self
                .interfaces
                .acquire()
                .expect("interface registry lock is infallible");
            let Some(interface_key) = interfaces
                .iter()
                .find(|(_, interface)| {
                    interface.device() == Some(device) && interface.family() == family
                })
                .map(|(key, _)| key)
            else {
                continue;
            };
            let Some(interface) = interfaces.get_mut(interface_key) else {
                return Err(StackError::InterfaceNotFound);
            };
            interface.input(frame.frame_type(), frame.data());
        }
        Ok(())
    }

    pub fn init() -> Result<(), <P as Platform>::Error> {
        info!("initialize...");
        let result = <P as Platform>::init();
        if result.is_err() {
            error!("failure");
            return result;
        }
        info!("success");
        result
    }

    pub fn shutdown() {
        info!("shutting down...");
        <P as Platform>::shutdown();
        info!("success");
    }
}
