use alloc::boxed::Box;
use core::any::Any;

use slotmap::{SlotMap, new_key_type};

use super::{AddressFamily, InterfaceError, NetInterface};
use crate::{DeviceKey, Lock, Platform};

new_key_type! {
    /// Stable key for an interface owned by an [`InterfaceRegistry`].
    pub struct InterfaceKey;
}

#[derive(Debug)]
pub struct InterfaceRegistry<P: Platform> {
    interfaces: P::Mutex<SlotMap<InterfaceKey, Box<dyn NetInterface<P>>>>,
}

type Interfaces<P> = SlotMap<InterfaceKey, Box<dyn NetInterface<P>>>;
type InterfaceGuard<'a, P> =
    <<P as Platform>::Mutex<Interfaces<P>> as Lock<Interfaces<P>>>::Guard<'a>;
type InterfaceLockError<P> = <<P as Platform>::Mutex<Interfaces<P>> as Lock<Interfaces<P>>>::Error;

impl<P: Platform> Default for InterfaceRegistry<P> {
    fn default() -> Self {
        Self {
            interfaces: P::Mutex::new(SlotMap::default()),
        }
    }
}

impl<P: Platform + 'static> InterfaceRegistry<P> {
    pub fn acquire(&self) -> Result<InterfaceGuard<'_, P>, InterfaceLockError<P>> {
        self.interfaces.acquire()
    }

    pub fn register(&self, interface: impl NetInterface<P> + 'static) -> InterfaceKey {
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
