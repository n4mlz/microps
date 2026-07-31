use getset::{CopyGetters, Getters};
use thiserror::Error;

use super::{Ipv4Addr, Ipv4Packet};
use crate::{
    AddressFamily, DeviceKey, DeviceRegistry, InterfaceError, InterfaceOutputError, NetInterface,
    Random, debug, error,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct Ipv4Interface {
    device: Option<DeviceKey>,
    #[getset(get_copy = "pub")]
    unicast: Ipv4Addr,
    #[getset(get_copy = "pub")]
    netmask: Ipv4Addr,
    #[getset(get_copy = "pub")]
    broadcast: Ipv4Addr,
}

impl Ipv4Interface {
    pub fn new(unicast: Ipv4Addr, netmask: Ipv4Addr) -> Self {
        let unicast = *unicast.as_bytes();
        let netmask = *netmask.as_bytes();
        let broadcast =
            core::array::from_fn(|index| (unicast[index] & netmask[index]) | !netmask[index]);

        Self {
            device: None,
            unicast: Ipv4Addr::from(unicast),
            netmask: Ipv4Addr::from(netmask),
            broadcast: Ipv4Addr::from(broadcast),
        }
    }

    pub fn output<R: Random>(
        &self,
        devices: &mut DeviceRegistry,
        protocol: u8,
        data: &[u8],
        source: Ipv4Addr,
        destination: Ipv4Addr,
    ) -> Result<usize, Ipv4OutputError<R::Error>> {
        if source != self.unicast {
            return Err(Ipv4OutputError::SourceNotOwned);
        }
        let destination_is_broadcast =
            destination == self.broadcast || destination == Ipv4Addr::BROADCAST;
        let same_network = source
            .as_bytes()
            .iter()
            .zip(destination.as_bytes())
            .zip(self.netmask.as_bytes())
            .all(|((source, destination), netmask)| source & netmask == destination & netmask);
        if !destination_is_broadcast && !same_network {
            return Err(Ipv4OutputError::DestinationUnreachable);
        }

        let id = R::random16().map_err(Ipv4OutputError::Random)?;
        let packet = Ipv4Packet::build(protocol, data, id, source, destination)?;
        self.output_raw(devices, super::TYPE, &packet, None)?;
        Ok(packet.len())
    }
}

#[derive(Debug, Error)]
pub enum Ipv4OutputError<E> {
    #[error("output source address does not belong to the interface")]
    SourceNotOwned,
    #[error("output destination is not reachable")]
    DestinationUnreachable,
    #[error("interface output failed: {0}")]
    Interface(#[from] InterfaceOutputError),
    #[error("packet construction failed: {0}")]
    Packet(#[from] super::Ipv4Error),
    #[error("random number generation failed")]
    Random(E),
}

impl NetInterface for Ipv4Interface {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn family(&self) -> AddressFamily {
        AddressFamily::Ipv4
    }

    fn input(&mut self, data: &[u8]) {
        let packet = match Ipv4Packet::try_from(data) {
            Ok(packet) => packet,
            Err(error) => {
                error!("{error}");
                return;
            }
        };
        let header = packet.header();
        let destination = header.destination();
        if destination != self.unicast
            && destination != self.broadcast
            && destination != Ipv4Addr::BROADCAST
        {
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
    }

    fn has_address(&self, address: &[u8]) -> bool {
        let Ok(address) = <[u8; 4]>::try_from(address) else {
            return false;
        };
        Ipv4Addr::from(address) == self.unicast
    }

    fn accepts(&self, address: &[u8]) -> bool {
        let Ok(address) = <[u8; 4]>::try_from(address) else {
            return false;
        };
        let address = Ipv4Addr::from(address);
        address == self.unicast || address == self.broadcast || address == Ipv4Addr::BROADCAST
    }

    fn device(&self) -> Option<DeviceKey> {
        self.device
    }

    fn attach(&mut self, device: DeviceKey) -> Result<(), InterfaceError> {
        if self.device.is_some() {
            return Err(InterfaceError::AlreadyAttached);
        }
        self.device = Some(device);
        Ok(())
    }

    fn detach(&mut self) -> Option<DeviceKey> {
        self.device.take()
    }
}
