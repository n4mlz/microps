use alloc::boxed::Box;
use core::any::Any;

use slotmap::{SlotMap, new_key_type};
use thiserror::Error;

use crate::{DeviceError, DeviceKey, DeviceRegistry, Platform};

new_key_type! {
    /// Stable key for an interface owned by an [`InterfaceRegistry`].
    pub struct InterfaceKey;
}

/// Address category understood by an interface.
///
/// This is a classification only. It does not identify an interface and does
/// not impose a one-interface-per-category restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

pub trait NetInterface: core::fmt::Debug + Send {
    fn as_any(&self) -> &dyn Any;

    fn family(&self) -> AddressFamily;

    fn input(&mut self, data: &[u8]);

    fn has_address(&self, address: &[u8]) -> bool;

    fn device(&self) -> Option<DeviceKey>;

    fn attach(&mut self, device: DeviceKey) -> Result<(), InterfaceError>;

    fn detach(&mut self) -> Option<DeviceKey>;

    fn accepts(&self, address: &[u8]) -> bool;

    fn output_raw<P: Platform>(
        &self,
        devices: &mut DeviceRegistry<P>,
        frame_type: u16,
        data: &[u8],
        destination: Option<&[u8]>,
    ) -> Result<(), InterfaceOutputError>
    where
        Self: Sized,
    {
        let device = self.device().ok_or(InterfaceOutputError::NotAttached)?;
        devices
            .device_mut(device)
            .ok_or(InterfaceOutputError::DeviceNotFound)?
            .output(frame_type, data, destination)
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

#[derive(Debug, Default)]
pub struct InterfaceRegistry {
    interfaces: SlotMap<InterfaceKey, Box<dyn NetInterface>>,
}

impl InterfaceRegistry {
    pub fn register(&mut self, interface: impl NetInterface + 'static) -> InterfaceKey {
        self.interfaces.insert(Box::new(interface))
    }

    pub fn interface(&self, key: InterfaceKey) -> Option<&dyn NetInterface> {
        self.interfaces.get(key).map(Box::as_ref)
    }

    pub fn interface_mut(&mut self, key: InterfaceKey) -> Option<&mut (dyn NetInterface + '_)> {
        self.interfaces
            .get_mut(key)
            .map(|interface| &mut **interface as &mut dyn NetInterface)
    }

    pub fn interface_as<T: Any>(&self, key: InterfaceKey) -> Option<&T> {
        self.interface(key)?.as_any().downcast_ref()
    }

    pub fn attach(
        &mut self,
        interface: InterfaceKey,
        device: DeviceKey,
    ) -> Result<(), InterfaceError> {
        self.interface_mut(interface)
            .ok_or(InterfaceError::NotFound)?
            .attach(device)
    }

    pub fn device(&self, interface: InterfaceKey) -> Option<DeviceKey> {
        self.interface(interface).and_then(NetInterface::device)
    }

    /// Returns the first matching interface for the book's one-interface
    /// simplification. Multiple matching interfaces remain valid.
    pub fn first_for_device(
        &self,
        device: DeviceKey,
        family: AddressFamily,
    ) -> Option<InterfaceKey> {
        self.interfaces
            .iter()
            .find(|(_, interface)| {
                interface.device() == Some(device) && interface.family() == family
            })
            .map(|(key, _)| key)
    }

    /// Selects the interface that owns an address.
    pub fn select_by_address(&self, address: &[u8]) -> Option<InterfaceKey> {
        self.interfaces
            .iter()
            .find(|(_, interface)| interface.has_address(address))
            .map(|(key, _)| key)
    }
}
