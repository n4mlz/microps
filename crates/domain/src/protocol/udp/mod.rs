mod header;
mod packet;
mod registry;

pub use header::*;
pub use packet::*;
pub use registry::*;
use thiserror::Error;

use super::Ipv4Packet;
use crate::{Platform, debug, debugdump};

pub const UDP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UdpInputError {
    #[error("invalid UDP packet: {0}")]
    Packet(#[from] UdpError),
    #[error("UDP destination port is unreachable")]
    PortUnreachable,
}

pub struct Udp;

impl Udp {
    pub fn input<P: Platform + 'static>(packet: Ipv4Packet<'_>) -> Result<(), UdpInputError> {
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
        let pcb = P::stack().udp_pcbs.select(packet.dest());
        let Some(pcb) = pcb else {
            return Err(UdpInputError::PortUnreachable);
        };
        let datagram = ReceivedDatagram::new(packet.src(), packet.payload().to_vec());
        if let Err(error) = P::stack().udp_pcbs.enqueue(pcb, datagram) {
            crate::error!("{error}");
        }
        Ok(())
    }
}
