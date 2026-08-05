use core::any::Any;

use thiserror::Error;

use crate::{DeviceError, DeviceKey, Platform};

mod registry;

pub use registry::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

pub trait NetInterface<P: Platform + 'static>: core::fmt::Debug + Send {
    fn as_any(&self) -> &dyn Any;
    fn family(&self) -> AddressFamily;
    fn input(&mut self, frame_type: u16, data: &[u8]);
    fn has_address(&self, address: &[u8]) -> bool;
    fn device(&self) -> Option<DeviceKey>;
    fn attach(&mut self, device: DeviceKey) -> Result<(), InterfaceError>;
    fn detach(&mut self) -> Option<DeviceKey>;
    fn accepts(&self, address: &[u8]) -> bool;

    fn output_raw(
        &self,
        frame_type: u16,
        data: &[u8],
        destination: Option<&[u8]>,
    ) -> Result<(), InterfaceOutputError> {
        let device = self.device().ok_or(InterfaceOutputError::NotAttached)?;
        P::stack()
            .devices
            .output(device, frame_type, data, destination)
            .map_err(InterfaceOutputError::Device)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InterfaceOutputError {
    #[error("interface is not attached to a device")]
    NotAttached,
    #[error("device does not exist")]
    DeviceNotFound,
    #[error("device operation failed: {0}")]
    Device(DeviceError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum InterfaceError {
    #[error("interface does not exist")]
    NotFound,
    #[error("interface is already attached to a device")]
    AlreadyAttached,
}
