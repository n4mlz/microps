mod addr;
mod header;
mod interface;
mod packet;

pub use addr::*;
pub use header::*;
pub use interface::*;
pub use packet::*;

use crate::{
    InterfaceOutputError, Platform, Random, debug, error,
    protocol::{EtherType, Icmp, IcmpType},
};

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
const IP_HEADER_LEN: usize = 20;

/// IPv4 protocol operations that do not require an instance.
pub struct Ipv4;

impl Ipv4 {
    pub fn output<P: Platform + 'static, R: Random>(
        interface: &Ipv4Interface,
        protocol: u8,
        data: &[u8],
        source: Ipv4Addr,
        destination: Ipv4Addr,
    ) -> Result<usize, Ipv4OutputError<R::Error>> {
        if source != interface.unicast() {
            return Err(Ipv4OutputError::SourceNotOwned);
        }
        let same_network = source
            .as_bytes()
            .iter()
            .zip(destination.as_bytes())
            .zip(interface.netmask().as_bytes())
            .all(|((source, destination), netmask)| source & netmask == destination & netmask);
        if destination != interface.broadcast()
            && destination != Ipv4Addr::BROADCAST
            && !same_network
        {
            return Err(Ipv4OutputError::DestinationUnreachable);
        }
        let id = R::random16().map_err(Ipv4OutputError::Random)?;
        let packet = Ipv4Packet::build(protocol, data, id, source, destination)?;
        let device = interface
            .device()
            .ok_or(InterfaceOutputError::NotAttached)?;
        P::stack()
            .devices
            .output(device, EtherType::Ipv4 as u16, &packet, None)
            .map_err(InterfaceOutputError::Device)?;
        Ok(packet.len())
    }

    pub fn input<P: Platform + 'static>(data: &[u8], interface: &Ipv4Interface) {
        let packet = match Ipv4Packet::try_from(data) {
            Ok(packet) => packet,
            Err(error) => {
                error!("{error}");
                return;
            }
        };
        let header = packet.header();
        if !interface.accepts(header.destination().as_bytes()) {
            return;
        }

        debug!(
            "vhl: 0x{:02x} [v: {}, hl: 5 (20)]",
            data[0],
            header.version()
        );
        debug!("tos: 0x{:02x}", header.tos());
        debug!(
            "total: {} (payload: {})",
            packet.packet_len(),
            packet.payload().len()
        );
        debug!("id: {}", header.id());
        debug!(
            "offset: 0x{:04x} [flags={}, offset={}]",
            (u16::from(header.flags()) << 13) | header.fragment_offset(),
            header.flags(),
            header.fragment_offset()
        );
        debug!("ttl: {}", header.ttl());
        debug!("protocol: {}", header.protocol());
        debug!("sum: {:?}", header.checksum());
        debug!("src: {}", header.source());
        debug!("dst: {}", header.destination());

        if let Ok(Ipv4Protocol::Icmp) = Ipv4Protocol::try_from(header.protocol()) {
            match Icmp::input(packet) {
                Ok(packet) if packet.header().type_value() == IcmpType::Echo as u8 => {
                    let message = Icmp::output(
                        IcmpType::EchoReply as u8,
                        packet.header().code(),
                        packet.header().dependent(),
                        packet.payload(),
                    );
                    if let Err(error) = Self::output::<P, P>(
                        interface,
                        Ipv4Protocol::Icmp as u8,
                        &message,
                        interface.unicast(),
                        packet.source(),
                    ) {
                        error!("{error}");
                    }
                }
                Ok(_) => {}
                Err(error) => error!("{error}"),
            }
        } else if data.len() >= IP_HEADER_LEN + crate::protocol::ICMP_HEADER_LEN {
            let offending = &data[..IP_HEADER_LEN + crate::protocol::ICMP_HEADER_LEN];
            let message = Icmp::output(IcmpType::DestinationUnreachable as u8, 2, 0, offending);
            if let Err(error) = Self::output::<P, P>(
                interface,
                Ipv4Protocol::Icmp as u8,
                &message,
                interface.unicast(),
                header.source(),
            ) {
                error!("{error}");
            }
        }
    }
}
