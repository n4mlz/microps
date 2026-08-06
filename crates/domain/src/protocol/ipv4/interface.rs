use getset::{CopyGetters, Getters};
use thiserror::Error;

use super::{Ipv4, Ipv4Addr};
use crate::{
    AddressFamily, DeviceKey, InterfaceError, InterfaceOutputError, NetInterface, Platform, Random,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct Ipv4Interface {
    device: Option<DeviceKey>,
    #[getset(get_copy = "pub")]
    unicast: Ipv4Addr,
    #[getset(get_copy = "pub")]
    netmask: Ipv4Addr,
    #[getset(get_copy = "pub")]
    broadcast: Ipv4Addr,
}

impl Ipv4Interface {
    pub fn new(unicast: Ipv4Addr, netmask: Ipv4Addr) -> Self {
        let unicast = *unicast.as_bytes();
        let netmask = *netmask.as_bytes();
        let broadcast =
            core::array::from_fn(|index| (unicast[index] & netmask[index]) | !netmask[index]);

        Self {
            device: None,
            unicast: Ipv4Addr::from(unicast),
            netmask: Ipv4Addr::from(netmask),
            broadcast: Ipv4Addr::from(broadcast),
        }
    }

    pub fn output<P: Platform + 'static, R: Random>(
        &self,
        protocol: u8,
        data: &[u8],
        src: Ipv4Addr,
        dst: Ipv4Addr,
    ) -> Result<usize, Ipv4OutputError<R::Error>> {
        Ipv4::output::<P, R>(self, protocol, data, src, dst)
    }

    pub fn accepts(&self, address: &[u8]) -> bool {
        let Ok(address) = <[u8; 4]>::try_from(address) else {
            return false;
        };
        let address = Ipv4Addr::from(address);
        address == self.unicast || address == self.broadcast || address == Ipv4Addr::BROADCAST
    }

    pub fn device(&self) -> Option<DeviceKey> {
        self.device
    }

    pub fn hardware_address<P: Platform + 'static>(&self) -> Option<crate::protocol::MacAddr> {
        let device = self.device?;
        P::stack()
            .devices
            .acquire()
            .expect("device registry lock is infallible")
            .get(device)
            .and_then(crate::Device::hardware_address)
    }
}

#[derive(Debug, Error)]
pub enum Ipv4OutputError<E> {
    #[error("a source address is required for the broadcast destination")]
    SourceRequiredForBroadcast,
    #[error("output source address does not belong to the interface")]
    SourceNotOwned,
    #[error("output destination is not reachable")]
    DestinationUnreachable,
    #[error("ARP resolution is incomplete")]
    ArpIncomplete,
    #[error("ARP request failed: {0}")]
    Arp(#[from] crate::protocol::ArpOutputError),
    #[error("interface output failed: {0}")]
    Interface(#[from] InterfaceOutputError),
    #[error("packet construction failed: {0}")]
    Packet(#[from] super::Ipv4Error),
    #[error("random number generation failed")]
    Random(E),
}

impl<P: Platform + 'static> NetInterface<P> for Ipv4Interface {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn family(&self) -> AddressFamily {
        AddressFamily::Ipv4
    }

    fn input(&mut self, frame_type: u16, data: &[u8]) {
        match frame_type {
            value if value == crate::protocol::EtherType::Ipv4 as u16 => {
                Ipv4::input::<P>(data, self)
            }
            value if value == crate::protocol::EtherType::Arp as u16 => {
                if let Err(error) = crate::protocol::Arp::input::<P>(data, self) {
                    crate::error!("{error}");
                }
            }
            _ => {}
        }
    }

    fn has_address(&self, address: &[u8]) -> bool {
        let Ok(address) = <[u8; 4]>::try_from(address) else {
            return false;
        };
        Ipv4Addr::from(address) == self.unicast
    }

    fn accepts(&self, address: &[u8]) -> bool {
        self.accepts(address)
    }

    fn device(&self) -> Option<DeviceKey> {
        self.device()
    }

    fn attach(&mut self, device: DeviceKey) -> Result<(), InterfaceError> {
        if self.device.is_some() {
            return Err(InterfaceError::AlreadyAttached);
        }
        self.device = Some(device);
        Ok(())
    }

    fn detach(&mut self) -> Option<DeviceKey> {
        self.device.take()
    }
}
