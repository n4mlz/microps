use getset::{CopyGetters, Getters};

use super::{Ipv4Addr, Ipv4Packet};
use crate::{debug, debugdump};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct IcmpPacket<'a> {
    #[getset(get_copy = "pub")]
    source: Ipv4Addr,
    #[getset(get_copy = "pub")]
    destination: Ipv4Addr,
    #[getset(get = "pub")]
    payload: &'a [u8],
}

impl<'a> IcmpPacket<'a> {
    pub fn from_ipv4(packet: Ipv4Packet<'a>) -> Self {
        let header = packet.header();
        Self {
            source: header.source(),
            destination: header.destination(),
            payload: packet.payload(),
        }
    }

    pub fn input(&self) {
        debug!(
            "{} => {}, len={}",
            self.source,
            self.destination,
            self.payload.len()
        );
        debugdump(self.payload);
    }
}
