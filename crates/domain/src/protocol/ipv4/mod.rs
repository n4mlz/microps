mod addr;
mod header;
mod icmp;
mod interface;
mod packet;

pub use addr::{Ipv4Addr, Ipv4AddrParseError};
pub use header::{Ipv4Error, Ipv4Header};
pub use icmp::{IcmpError, IcmpHeader, IcmpPacket, IcmpType, UnknownIcmpType};
pub use interface::{Ipv4Interface, Ipv4OutputError};
pub use packet::Ipv4Packet;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ipv4Protocol {
    Icmp = 1,
    Tcp = 6,
    Udp = 17,
}

impl TryFrom<u8> for Ipv4Protocol {
    type Error = UnknownIpv4Protocol;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            value if value == Self::Icmp as u8 => Ok(Self::Icmp),
            value if value == Self::Tcp as u8 => Ok(Self::Tcp),
            value if value == Self::Udp as u8 => Ok(Self::Udp),
            value => Err(UnknownIpv4Protocol(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownIpv4Protocol(pub u8);

/// IPv4 version carried in the high four bits of the first header byte.
const VERSION: u8 = 4;

/// Length of the IPv4 base header in bytes; options are not supported yet.
const HEADER_LEN: usize = 20;
