mod header;
mod packet;

pub use header::*;
pub use packet::*;

use super::Ipv4Packet;
use crate::{debug, debugdump};

pub const UDP_HEADER_LEN: usize = 8;

pub struct Udp;

impl Udp {
    pub fn input(packet: Ipv4Packet<'_>) -> Result<(), UdpError> {
        let packet = UdpPacket::from_ipv4(packet)?;
        debug!(
            "{} => {}, len={}",
            packet.src(),
            packet.dest(),
            packet.data().len()
        );
        debug!("src: {}", packet.header().src_port());
        debug!("dst: {}", packet.header().dest_port());
        debug!(
            "len: {} (payload: {})",
            packet.header().length(),
            packet.payload().len()
        );
        debug!("sum: 0x{:04x}", packet.header().checksum());
        debugdump(packet.data());
        Ok(())
    }
}
