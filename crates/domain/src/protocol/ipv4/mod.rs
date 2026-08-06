mod addr;
mod header;
mod interface;
mod packet;
mod route;

pub use addr::*;
pub use header::*;
pub use interface::*;
pub use packet::*;
pub use route::*;

use crate::{
    NetInterface, Platform, Random, debug, error,
    protocol::{Arp, EtherType, Icmp, MacAddr, Tcp, Udp},
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
        src: Ipv4Addr,
        dest: Ipv4Addr,
    ) -> Result<usize, Ipv4OutputError<R::Error>> {
        if src == Ipv4Addr::ANY && dest == Ipv4Addr::BROADCAST {
            return Err(Ipv4OutputError::SourceRequiredForBroadcast);
        }
        let route = P::stack()
            .ipv4_routes
            .lookup(dest)
            .ok_or(Ipv4OutputError::DestinationUnreachable)?;
        let routed_interface = P::stack()
            .interfaces
            .interface_as::<Ipv4Interface>(route.interface())
            .ok_or(Ipv4OutputError::DestinationUnreachable)?;
        if routed_interface.device() != interface.device() {
            return Err(Ipv4OutputError::SourceNotOwned);
        }
        if src != Ipv4Addr::ANY && src != routed_interface.unicast() {
            return Err(Ipv4OutputError::SourceNotOwned);
        }
        let src = if src == Ipv4Addr::ANY {
            routed_interface.unicast()
        } else {
            src
        };
        let id = R::random16().map_err(Ipv4OutputError::Random)?;
        let packet = Ipv4Packet::build(protocol, data, id, src, dest)?;
        let dest_hardware = if routed_interface.hardware_address::<P>().is_some() {
            Some(
                if dest == routed_interface.broadcast() || dest == Ipv4Addr::BROADCAST {
                    MacAddr::BROADCAST
                } else {
                    Arp::resolve::<P>(
                        &routed_interface,
                        if route.nexthop() == Ipv4Addr::ANY {
                            dest
                        } else {
                            route.nexthop()
                        },
                    )
                    .map_err(|error| match error {
                        crate::protocol::ArpResolveError::Incomplete => {
                            Ipv4OutputError::ArpIncomplete
                        }
                        crate::protocol::ArpResolveError::Request(error) => {
                            Ipv4OutputError::Arp(error)
                        }
                    })?
                },
            )
        } else {
            None
        };
        let dest_bytes = dest_hardware.map(|address| address.bytes());
        <Ipv4Interface as NetInterface<P>>::output_raw(
            &routed_interface,
            EtherType::Ipv4 as u16,
            &packet,
            dest_bytes.as_ref().map(|bytes| &bytes[..]),
        )?;
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
        if !interface.accepts(header.dest().as_bytes()) {
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
        debug!("src: {}", header.src());
        debug!("dst: {}", header.dest());

        match Ipv4Protocol::try_from(header.protocol()) {
            Ok(Ipv4Protocol::Icmp) => {
                if let Err(error) = Icmp::input::<P, P>(packet, interface) {
                    error!("{error}");
                }
            }
            Ok(Ipv4Protocol::Udp) => {
                if let Err(error) = Udp::input::<P>(packet) {
                    match error {
                        crate::protocol::UdpInputError::PortUnreachable
                            if data.len() >= IP_HEADER_LEN + crate::protocol::UDP_HEADER_LEN =>
                        {
                            let offending =
                                &data[..IP_HEADER_LEN + crate::protocol::UDP_HEADER_LEN];
                            if let Err(error) =
                                Icmp::port_unreachable::<P, P>(interface, offending, header.src())
                            {
                                error!("{error}");
                            }
                        }
                        error => error!("{error}"),
                    }
                }
            }
            Ok(Ipv4Protocol::Tcp) => {
                if let Err(error) = Tcp::input(packet, interface) {
                    error!("{error}");
                }
            }
            _ if data.len() >= IP_HEADER_LEN + crate::protocol::ICMP_HEADER_LEN => {
                let offending = &data[..IP_HEADER_LEN + crate::protocol::ICMP_HEADER_LEN];
                if let Err(error) =
                    Icmp::destination_unreachable::<P, P>(interface, offending, header.src())
                {
                    error!("{error}");
                }
            }
            _ => {}
        }
    }
}
