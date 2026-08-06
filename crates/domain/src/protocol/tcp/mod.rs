mod header;
mod packet;

pub use header::*;
pub use packet::*;
use thiserror::Error;

use super::{Ipv4Addr, Ipv4Interface, Ipv4Packet};
use crate::{debug, debugdump};

pub const TCP_HEADER_LEN: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TcpInputError {
    #[error("invalid TCP segment: {0}")]
    Packet(#[from] TcpError),
    #[error("TCP segments must use unicast addresses")]
    Broadcast,
}

pub struct Tcp;

impl Tcp {
    pub fn input(packet: Ipv4Packet<'_>, interface: &Ipv4Interface) -> Result<(), TcpInputError> {
        let packet = TcpPacket::from_ipv4(packet)?;
        if packet.src().address() == Ipv4Addr::BROADCAST
            || packet.src().address() == interface.broadcast()
            || packet.dest().address() == Ipv4Addr::BROADCAST
            || packet.dest().address() == interface.broadcast()
        {
            return Err(TcpInputError::Broadcast);
        }

        debug!(
            "{} => {}, len={}, dev={:?}",
            packet.src(),
            packet.dest(),
            packet.data().len(),
            interface.device()
        );
        debug!("src: {}", packet.header().src_port());
        debug!("dst: {}", packet.header().dest_port());
        debug!("seq: {}", packet.header().sequence_number());
        debug!("ack: {}", packet.header().acknowledgment_number());
        debug!(
            "off: 0x{:02x} ({}), options: {}, payload: {}",
            packet.header().data_offset(),
            packet.header().header_len(),
            packet.options().len(),
            packet.payload().len()
        );
        debug!(
            "flg: 0x{:02x} ({:?})",
            packet.header().flags().bits(),
            packet.header().flags()
        );
        debug!("wnd: {}", packet.header().window_size());
        debug!("sum: 0x{:04x}", packet.header().checksum());
        debug!("up: {}", packet.header().urgent_pointer());
        debugdump(packet.data());
        Ok(())
    }
}
