mod cache;
mod header;
mod packet;

pub use cache::*;
pub use header::*;
pub use packet::*;
use thiserror::Error;

use crate::{
    NetInterface, Platform,
    protocol::{EtherType, Ipv4Addr, Ipv4Interface, MacAddr},
};

pub const PACKET_LEN: usize = 28;

pub struct Arp;

impl Arp {
    pub fn resolve<P: Platform + 'static>(protocol: Ipv4Addr) -> Option<MacAddr> {
        P::stack().arp_cache.resolve(protocol, P::now())
    }

    pub fn output<P: Platform + 'static>(
        interface: &Ipv4Interface,
        operation: ArpOperation,
        dest_hardware: MacAddr,
        dest_protocol: Ipv4Addr,
    ) -> Result<usize, ArpOutputError> {
        let src_hardware = interface
            .hardware_address::<P>()
            .ok_or(ArpOutputError::HardwareAddressUnavailable)?;
        let packet = ArpPacket::build(
            operation,
            src_hardware,
            interface.unicast(),
            dest_hardware,
            dest_protocol,
        );
        <Ipv4Interface as NetInterface<P>>::output_raw(
            interface,
            EtherType::Arp as u16,
            &packet,
            Some(&dest_hardware.bytes()),
        )
        .map_err(ArpOutputError::Interface)?;
        Ok(packet.len())
    }

    pub fn input<P: Platform + 'static>(
        data: &[u8],
        interface: &Ipv4Interface,
    ) -> Result<(), ArpError> {
        let packet = ArpPacket::try_from(data)?;
        let now = P::now();
        let updated =
            P::stack()
                .arp_cache
                .update(packet.sender_protocol(), packet.sender_hardware(), now);
        if !updated && packet.target_protocol() == interface.unicast() {
            P::stack()
                .arp_cache
                .insert(packet.sender_protocol(), packet.sender_hardware(), now);
        }
        if packet.target_protocol() != interface.unicast() {
            return Ok(());
        }
        if packet.header().operation() == ArpOperation::Request as u16
            && let Err(error) = Self::output::<P>(
                interface,
                ArpOperation::Reply,
                packet.sender_hardware(),
                packet.sender_protocol(),
            )
        {
            crate::error!("{error}");
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ArpOutputError {
    #[error("interface has no hardware address")]
    HardwareAddressUnavailable,
    #[error("interface output failed: {0}")]
    Interface(#[from] crate::InterfaceOutputError),
}
