use alloc::vec::Vec;

use getset::CopyGetters;
use thiserror::Error;

use crate::{
    NetInterface, Platform,
    protocol::{EtherType, Ipv4Addr, Ipv4Interface, MacAddr},
};

pub const PACKET_LEN: usize = 28;

pub struct Arp;

impl Arp {
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

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArpOperation {
    Request = 1,
    Reply = 2,
}

impl TryFrom<u16> for ArpOperation {
    type Error = UnknownArpOperation;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Request),
            2 => Ok(Self::Reply),
            value => Err(UnknownArpOperation(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownArpOperation(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct ArpHeader {
    #[getset(get_copy = "pub")]
    hardware_type: u16,
    #[getset(get_copy = "pub")]
    protocol_type: u16,
    #[getset(get_copy = "pub")]
    hardware_len: u8,
    #[getset(get_copy = "pub")]
    protocol_len: u8,
    #[getset(get_copy = "pub")]
    operation: u16,
}

impl TryFrom<&[u8]> for ArpHeader {
    type Error = ArpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < PACKET_LEN {
            return Err(ArpError::TooShort { len: data.len() });
        }
        let header = Self {
            hardware_type: u16::from_be_bytes([data[0], data[1]]),
            protocol_type: u16::from_be_bytes([data[2], data[3]]),
            hardware_len: data[4],
            protocol_len: data[5],
            operation: u16::from_be_bytes([data[6], data[7]]),
        };
        if header.hardware_type != 1 || header.hardware_len != 6 {
            return Err(ArpError::UnsupportedHardware {
                hardware_type: header.hardware_type,
                hardware_len: header.hardware_len,
            });
        }
        if header.protocol_type != EtherType::Ipv4 as u16 || header.protocol_len != 4 {
            return Err(ArpError::UnsupportedProtocol {
                protocol_type: header.protocol_type,
                protocol_len: header.protocol_len,
            });
        }
        Ok(header)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct ArpPacket {
    #[getset(get_copy = "pub")]
    header: ArpHeader,
    #[getset(get_copy = "pub")]
    sender_hardware: MacAddr,
    #[getset(get_copy = "pub")]
    sender_protocol: Ipv4Addr,
    #[getset(get_copy = "pub")]
    target_hardware: MacAddr,
    #[getset(get_copy = "pub")]
    target_protocol: Ipv4Addr,
}

impl ArpPacket {
    pub fn build(
        operation: ArpOperation,
        sender_hardware: MacAddr,
        sender_protocol: Ipv4Addr,
        target_hardware: MacAddr,
        target_protocol: Ipv4Addr,
    ) -> Vec<u8> {
        let mut data = Vec::with_capacity(PACKET_LEN);
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&(EtherType::Ipv4 as u16).to_be_bytes());
        data.extend_from_slice(&[6, 4]);
        data.extend_from_slice(&(operation as u16).to_be_bytes());
        data.extend_from_slice(&sender_hardware.bytes());
        data.extend_from_slice(sender_protocol.as_bytes());
        data.extend_from_slice(&target_hardware.bytes());
        data.extend_from_slice(target_protocol.as_bytes());
        data
    }
}

impl TryFrom<&[u8]> for ArpPacket {
    type Error = ArpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let header = ArpHeader::try_from(data)?;
        Ok(Self {
            header,
            sender_hardware: MacAddr::new(data[8..14].try_into().unwrap()),
            sender_protocol: Ipv4Addr::new(data[14..18].try_into().unwrap()),
            target_hardware: MacAddr::new(data[18..24].try_into().unwrap()),
            target_protocol: Ipv4Addr::new(data[24..28].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ArpError {
    #[error("ARP packet is too short: {len} bytes")]
    TooShort { len: usize },
    #[error("unsupported ARP hardware: type={hardware_type}, length={hardware_len}")]
    UnsupportedHardware {
        hardware_type: u16,
        hardware_len: u8,
    },
    #[error("unsupported ARP protocol: type=0x{protocol_type:04x}, length={protocol_len}")]
    UnsupportedProtocol {
        protocol_type: u16,
        protocol_len: u8,
    },
}

#[derive(Debug, Error)]
pub enum ArpOutputError {
    #[error("interface has no hardware address")]
    HardwareAddressUnavailable,
    #[error("interface output failed: {0}")]
    Interface(#[from] crate::InterfaceOutputError),
}
