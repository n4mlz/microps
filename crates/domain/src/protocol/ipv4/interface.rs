use super::Ipv4Addr;
use crate::{AddressFamily, NetInterface};

/// IPv4 address configuration belonging to a network interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ipv4Interface {
    unicast: Ipv4Addr,
    netmask: Ipv4Addr,
    broadcast: Ipv4Addr,
}

impl Ipv4Interface {
    pub fn new(unicast: Ipv4Addr, netmask: Ipv4Addr) -> Self {
        let unicast_octets = unicast.octets();
        let netmask_octets = netmask.octets();
        let mut broadcast_octets = [0; 4];

        for index in 0..broadcast_octets.len() {
            broadcast_octets[index] =
                (unicast_octets[index] & netmask_octets[index]) | !netmask_octets[index];
        }

        Self {
            unicast,
            netmask,
            broadcast: Ipv4Addr::from(broadcast_octets),
        }
    }

    pub fn unicast(&self) -> Ipv4Addr {
        self.unicast
    }

    pub fn netmask(&self) -> Ipv4Addr {
        self.netmask
    }

    pub fn broadcast(&self) -> Ipv4Addr {
        self.broadcast
    }

    pub fn accepts(&self, address: &[u8]) -> bool {
        if address.len() != 4 {
            return false;
        }

        let address = Ipv4Addr::from([address[0], address[1], address[2], address[3]]);
        address == self.unicast || address == self.broadcast || address == Ipv4Addr::BROADCAST
    }
}

impl NetInterface for Ipv4Interface {
    fn family(&self) -> AddressFamily {
        AddressFamily::Ipv4
    }

    fn accepts(&self, address: &[u8]) -> bool {
        self.accepts(address)
    }
}
