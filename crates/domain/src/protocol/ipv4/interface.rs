use getset::{CopyGetters, Getters};

use super::{Ipv4Addr, Ipv4Packet};
use crate::{AddressFamily, DeviceKey, InterfaceError, NetInterface, debug, error};

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
        let unicast = unicast.octets();
        let netmask = netmask.octets();
        let broadcast =
            core::array::from_fn(|index| (unicast[index] & netmask[index]) | !netmask[index]);

        Self {
            device: None,
            unicast: Ipv4Addr::from(unicast),
            netmask: Ipv4Addr::from(netmask),
            broadcast: Ipv4Addr::from(broadcast),
        }
    }
}

impl NetInterface for Ipv4Interface {
    fn family(&self) -> AddressFamily {
        AddressFamily::Ipv4
    }

    fn input(&mut self, data: &[u8]) {
        let packet = match Ipv4Packet::try_from(data) {
            Ok(packet) => packet,
            Err(error) => {
                error!("{error}");
                return;
            }
        };
        let header = packet.header();
        let destination = header.destination();
        if destination != self.unicast
            && destination != self.broadcast
            && destination != Ipv4Addr::BROADCAST
        {
            return;
        }

        debug!(
            "vhl: 0x{:02x} [v: {}, hl: 5 (20)]",
            data[0],
            header.version()
        );
        debug!("tos: 0x{:02x}", header.tos());
        debug!(
            "total: {} (payload: {})",
            packet.total_len(),
            packet.payload().len()
        );
        debug!("id: {}", header.id());
        debug!(
            "offset: 0x{:04x} [flags={}, offset={}]",
            (u16::from(header.flags()) << 13) | header.fragment_offset(),
            header.flags(),
            header.fragment_offset()
        );
        debug!("ttl: {}", header.ttl());
        debug!("protocol: {}", header.protocol());
        debug!("sum: 0x{:04x}", header.checksum());
        debug!("src: {}", header.source());
        debug!("dst: {}", header.destination());
    }

    fn has_address(&self, address: &[u8]) -> bool {
        let Ok(address) = <[u8; 4]>::try_from(address) else {
            return false;
        };
        Ipv4Addr::from(address) == self.unicast
    }

    fn accepts(&self, address: &[u8]) -> bool {
        let Ok(address) = <[u8; 4]>::try_from(address) else {
            return false;
        };
        let address = Ipv4Addr::from(address);
        address == self.unicast || address == self.broadcast || address == Ipv4Addr::BROADCAST
    }

    fn device(&self) -> Option<DeviceKey> {
        self.device
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
