use alloc::{boxed::Box, vec::Vec};

// This registry is provisional while only IPv4 exists. When IPv6 is added,
// interface ownership and lookup should be redesigned with the protocol
// layer instead of exposing an IPv4/IPv6 enum from this common module.

/// Address family implemented by a network interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressFamily {
    Ipv4,
    Ipv6,
}

/// Common behavior required from a device's network interface.
pub trait NetInterface: core::fmt::Debug + Send + Sync {
    fn family(&self) -> AddressFamily;

    fn accepts(&self, address: &[u8]) -> bool;
}

/// Alloc-backed collection of network interfaces owned by a device.
#[derive(Debug, Default)]
pub struct InterfaceRegistry {
    interfaces: Vec<Box<dyn NetInterface>>,
}

impl InterfaceRegistry {
    pub fn new() -> Self {
        Self {
            interfaces: Vec::new(),
        }
    }

    pub fn add(&mut self, interface: impl NetInterface + 'static) -> Result<(), AddressFamily> {
        let interface = Box::new(interface);
        let family = interface.family();
        if self
            .interfaces
            .iter()
            .any(|current| current.family() == family)
        {
            return Err(family);
        }
        self.interfaces.push(interface);
        Ok(())
    }

    pub fn get(&self, family: AddressFamily) -> Option<&dyn NetInterface> {
        self.interfaces
            .iter()
            .find(|interface| interface.family() == family)
            .map(Box::as_ref)
    }
}
