use crate::{Device, NetInterface};

mod ipv4;

pub use ipv4::{Ipv4Addr, Ipv4AddrParseError, Ipv4Error, Ipv4Header, Ipv4Interface, Ipv4Packet};

pub const IPV4_TYPE: u16 = ipv4::TYPE;

impl Device {
    pub fn input(&mut self, interface: &mut dyn NetInterface, frame_type: u16, data: &[u8]) {
        if frame_type == ipv4::TYPE {
            interface.input(data);
        }
    }
}
