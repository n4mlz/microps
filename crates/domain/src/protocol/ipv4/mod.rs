mod addr;
mod header;
mod interface;
mod packet;

pub use addr::{Ipv4Addr, Ipv4AddrParseError};
pub use header::{Ipv4Error, Ipv4Header};
pub use interface::{Ipv4Interface, Ipv4OutputError};
pub use packet::Ipv4Packet;

/// IPv4 version carried in the high four bits of the first header byte.
const VERSION: u8 = 4;

/// Length of the IPv4 base header in bytes; options are not supported yet.
const HEADER_LEN: usize = 20;
