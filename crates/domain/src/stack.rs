use getset::Getters;
use thiserror::Error;

use crate::{
    AddressFamily, Device, DeviceBackend, DeviceError, DeviceKey, DeviceMeta, DeviceRegistry,
    InputQueue, InterfaceError, InterfaceRegistry, Platform, debug, debugdump, error, info,
    protocol,
};

/// Network stack state and ownership root for devices and interfaces.
#[derive(Getters, Default)]
pub struct Stack<P: Platform> {
    pub devices: DeviceRegistry<P>,
    pub interfaces: InterfaceRegistry,
    #[getset(get = "pub")]
    input_queue: InputQueue<P>,
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

impl<P: Platform> Stack<P> {
    pub fn new() -> Self {
        Self {
            devices: DeviceRegistry::default(),
            interfaces: InterfaceRegistry::default(),
            input_queue: alloc::sync::Arc::default(),
        }
    }

    pub fn register_device(
        &mut self,
        meta: DeviceMeta,
        backend: impl DeviceBackend<P> + 'static,
    ) -> DeviceKey {
        self.devices.register(Device::new(meta, backend))
    }

    pub fn open_all(&mut self) -> Result<(), StackError> {
        self.devices.open_all()?;
        Ok(())
    }

    pub fn close_all(&mut self) {
        self.devices.close_all();
    }

    pub fn soft_input(&mut self) -> Result<(), StackError> {
        while let Some(frame) = self.input_queue.pop() {
            let device = frame.device();
            if self.devices.device(device).is_none() {
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
                _ => {
                    continue;
                }
            };
            let Some(interface_key) = self.interfaces.first_for_device(device, family) else {
                continue;
            };
            let Some(interface) = self.interfaces.interface_mut(interface_key) else {
                return Err(StackError::InterfaceNotFound);
            };
            interface.input(frame.data());
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
