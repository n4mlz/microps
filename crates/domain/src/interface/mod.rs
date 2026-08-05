use alloc::boxed::Box;
use core::any::Any;

use slotmap::{SlotMap, new_key_type};
use thiserror::Error;

use crate::{DeviceError, DeviceKey, Lock, Platform};

new_key_type! {
    /// Stable key for an interface owned by an [`InterfaceRegistry`].
    pub struct InterfaceKey;
}

/// Address category understood by an interface.
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

    fn output_raw<P: Platform + 'static>(
        &self,
        frame_type: u16,
        data: &[u8],
        destination: Option<&[u8]>,
    ) -> Result<(), InterfaceOutputError>
    where
        Self: Sized,
    {
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

#[derive(Debug)]
pub struct InterfaceRegistry<P: Platform> {
    interfaces: P::Mutex<SlotMap<InterfaceKey, Box<dyn NetInterface>>>,
}

type Interfaces = SlotMap<InterfaceKey, Box<dyn NetInterface>>;
type InterfaceGuard<'a, P> = <<P as Platform>::Mutex<Interfaces> as Lock<Interfaces>>::Guard<'a>;
type InterfaceLockError<P> = <<P as Platform>::Mutex<Interfaces> as Lock<Interfaces>>::Error;

impl<P: Platform> Default for InterfaceRegistry<P> {
    fn default() -> Self {
        Self {
            interfaces: P::Mutex::new(SlotMap::default()),
        }
    }
}

impl<P: Platform> InterfaceRegistry<P> {
    pub fn acquire(&self) -> Result<InterfaceGuard<'_, P>, InterfaceLockError<P>> {
        self.interfaces.acquire()
    }

    pub fn register(&self, interface: impl NetInterface + 'static) -> InterfaceKey {
        self.interfaces
            .acquire()
            .expect("interface registry lock is infallible")
            .insert(Box::new(interface))
    }

    pub fn interface_as<T: Any + Clone>(&self, key: InterfaceKey) -> Option<T> {
        self.interfaces
            .acquire()
            .expect("interface registry lock is infallible")
            .get(key)
            .and_then(|interface| interface.as_any().downcast_ref::<T>().cloned())
    }

    pub fn attach(&self, interface: InterfaceKey, device: DeviceKey) -> Result<(), InterfaceError> {
        self.interfaces
            .acquire()
            .expect("interface registry lock is infallible")
            .get_mut(interface)
            .ok_or(InterfaceError::NotFound)?
            .attach(device)
    }

    pub fn device(&self, interface: InterfaceKey) -> Option<DeviceKey> {
        self.interfaces
            .acquire()
            .expect("interface registry lock is infallible")
            .get(interface)
            .and_then(|interface| interface.device())
    }

    pub fn first_for_device(
        &self,
        device: DeviceKey,
        family: AddressFamily,
    ) -> Option<InterfaceKey> {
        self.interfaces
            .acquire()
            .expect("interface registry lock is infallible")
            .iter()
            .find(|(_, interface)| {
                interface.device() == Some(device) && interface.family() == family
            })
            .map(|(key, _)| key)
    }

    pub fn select_by_address(&self, address: &[u8]) -> Option<InterfaceKey> {
        self.interfaces
            .acquire()
            .expect("interface registry lock is infallible")
            .iter()
            .find(|(_, interface)| interface.has_address(address))
            .map(|(key, _)| key)
    }
}
