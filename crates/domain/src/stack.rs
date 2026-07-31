use thiserror::Error;

use crate::{
    AddressFamily, DeviceError, DeviceKey, DeviceRegistry, InterfaceError, InterfaceRegistry,
    Platform, debug, debugdump, error, info, protocol,
};

/// Network stack state and ownership root for devices and interfaces.
#[derive(Debug, Default)]
pub struct Stack {
    pub devices: DeviceRegistry,
    pub interfaces: InterfaceRegistry,
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

impl Stack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_all(&mut self) -> Result<(), StackError> {
        self.devices.open_all()?;
        Ok(())
    }

    pub fn close_all(&mut self) {
        self.devices.close_all();
    }

    pub fn input(
        &mut self,
        device: DeviceKey,
        frame_type: u16,
        data: &[u8],
    ) -> Result<(), StackError> {
        if self.devices.device(device).is_none() {
            return Err(StackError::DeviceNotFound);
        }
        debug!(
            "device={device:?}, type=0x{frame_type:04x}, len={}",
            data.len()
        );
        debugdump(data);
        let family = match frame_type {
            type_value if type_value == protocol::EtherType::Ipv4 as u16 => AddressFamily::Ipv4,
            _ => return Ok(()),
        };
        let Some(interface_key) = self.interfaces.first_for_device(device, family) else {
            return Ok(());
        };
        let Some(interface) = self.interfaces.interface_mut(interface_key) else {
            return Err(StackError::InterfaceNotFound);
        };
        self.devices
            .device_mut(device)
            .ok_or(StackError::DeviceNotFound)?
            .input(interface, frame_type, data);
        Ok(())
    }

    pub fn init<P: Platform>() -> Result<(), P::Error> {
        info!("initialize...");
        let result = P::init();
        if result.is_err() {
            error!("failure");
            return result;
        }
        info!("success");
        result
    }

    pub fn shutdown<P: Platform>() {
        info!("shutting down...");
        P::shutdown();
        info!("success");
    }
}
