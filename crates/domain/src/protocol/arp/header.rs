use getset::CopyGetters;
use thiserror::Error;

use super::PACKET_LEN;
use crate::protocol::EtherType;

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

impl TryFrom<&[u8]> for ArpHeader {
    type Error = super::ArpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < PACKET_LEN {
            return Err(super::ArpError::TooShort { len: data.len() });
        }
        let header = Self {
            hardware_type: u16::from_be_bytes([data[0], data[1]]),
            protocol_type: u16::from_be_bytes([data[2], data[3]]),
            hardware_len: data[4],
            protocol_len: data[5],
            operation: u16::from_be_bytes([data[6], data[7]]),
        };
        if header.hardware_type != 1 || header.hardware_len != 6 {
            return Err(super::ArpError::UnsupportedHardware {
                hardware_type: header.hardware_type,
                hardware_len: header.hardware_len,
            });
        }
        if header.protocol_type != EtherType::Ipv4 as u16 || header.protocol_len != 4 {
            return Err(super::ArpError::UnsupportedProtocol {
                protocol_type: header.protocol_type,
                protocol_len: header.protocol_len,
            });
        }
        Ok(header)
    }
}
