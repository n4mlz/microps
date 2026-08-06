mod header;
mod packet;
mod registry;

pub use header::*;
pub use packet::*;
pub use registry::*;
use thiserror::Error;

use super::{Ipv4Addr, Ipv4Endpoint, Ipv4Interface, Ipv4Packet, Ipv4Protocol};
use crate::{Platform, Random, debug, debugdump};

pub const UDP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UdpInputError {
    #[error("invalid UDP packet: {0}")]
    Packet(#[from] UdpError),
    #[error("UDP destination port is unreachable")]
    PortUnreachable,
}

#[derive(Debug, Error)]
pub enum UdpOutputError<E> {
    #[error("UDP PCB operation failed: {0}")]
    Pcb(#[from] UdpPcbError),
    #[error("UDP packet construction failed: {0}")]
    Packet(#[from] UdpError),
    #[error("IPv4 output failed: {0}")]
    Ipv4(#[from] crate::protocol::Ipv4OutputError<E>),
}

pub struct Udp;

impl Udp {
    pub fn send_to<P: Platform + 'static>(
        pcb: UdpPcbKey,
        payload: &[u8],
        remote: Ipv4Endpoint,
    ) -> Result<usize, UdpOutputError<<P as Random>::Error>> {
        let stack = P::stack();
        let local = stack.udp_pcbs.assign_dynamic_port(pcb)?;
        let interface_key = stack
            .ipv4_routes
            .lookup(remote.address())
            .ok_or(crate::protocol::Ipv4OutputError::DestinationUnreachable)?
            .interface();
        let interface = stack
            .interfaces
            .interface_as::<Ipv4Interface>(interface_key)
            .ok_or(crate::protocol::Ipv4OutputError::DestinationUnreachable)?;
        let source = if local.address() == Ipv4Addr::ANY {
            interface.unicast()
        } else {
            local.address()
        };
        let local = Ipv4Endpoint::new(source, local.port());
        let packet = UdpPacket::build(local, remote, payload)?;
        interface
            .output::<P, P>(Ipv4Protocol::Udp as u8, &packet, source, remote.address())
            .map(|_| payload.len())
            .map_err(UdpOutputError::Ipv4)
    }

    pub fn recv_from<P: Platform + 'static>(
        pcb: UdpPcbKey,
        buffer: &mut [u8],
    ) -> Result<(usize, Ipv4Endpoint), UdpPcbError> {
        P::stack().udp_pcbs.recv_from(pcb, buffer)
    }

    pub fn input<P: Platform + 'static>(packet: Ipv4Packet<'_>) -> Result<(), UdpInputError> {
        let packet = UdpPacket::from_ipv4(packet)?;
        debug!(
            "{} => {}, len={}",
            packet.src(),
            packet.dst(),
            packet.data().len()
        );
        debug!("src: {}", packet.header().src_port());
        debug!("dst: {}", packet.header().dst_port());
        debug!(
            "len: {} (payload: {})",
            packet.header().length(),
            packet.payload().len()
        );
        debug!("sum: 0x{:04x}", packet.header().checksum());
        debugdump(packet.data());
        let pcb = P::stack().udp_pcbs.select(packet.dst());
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
