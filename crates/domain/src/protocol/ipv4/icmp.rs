use core::fmt;

use getset::{CopyGetters, Getters};
use thiserror::Error;

use super::{Ipv4Addr, Ipv4Packet};
use crate::{debug, debugdump, protocol::checksum16};

pub const ICMP_HEADER_LEN: usize = 8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IcmpType {
    EchoReply = 0,
    DestinationUnreachable = 3,
    SourceQuench = 4,
    Redirect = 5,
    Echo = 8,
    TimeExceeded = 11,
    ParameterProblem = 12,
    Timestamp = 13,
    TimestampReply = 14,
    InformationRequest = 15,
    InformationReply = 16,
}

impl TryFrom<u8> for IcmpType {
    type Error = UnknownIcmpType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::EchoReply),
            3 => Ok(Self::DestinationUnreachable),
            4 => Ok(Self::SourceQuench),
            5 => Ok(Self::Redirect),
            8 => Ok(Self::Echo),
            11 => Ok(Self::TimeExceeded),
            12 => Ok(Self::ParameterProblem),
            13 => Ok(Self::Timestamp),
            14 => Ok(Self::TimestampReply),
            15 => Ok(Self::InformationRequest),
            16 => Ok(Self::InformationReply),
            value => Err(UnknownIcmpType(value)),
        }
    }
}

impl fmt::Display for IcmpType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::EchoReply => "EchoReply",
            Self::DestinationUnreachable => "DestinationUnreachable",
            Self::SourceQuench => "SourceQuench",
            Self::Redirect => "Redirect",
            Self::Echo => "Echo",
            Self::TimeExceeded => "TimeExceeded",
            Self::ParameterProblem => "ParameterProblem",
            Self::Timestamp => "Timestamp",
            Self::TimestampReply => "TimestampReply",
            Self::InformationRequest => "InformationRequest",
            Self::InformationReply => "InformationReply",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownIcmpType(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct IcmpHeader {
    #[getset(get_copy = "pub")]
    type_value: u8,
    #[getset(get_copy = "pub")]
    code: u8,
    #[getset(get_copy = "pub")]
    checksum: u16,
    #[getset(get_copy = "pub")]
    dependent: u32,
}

impl TryFrom<&[u8]> for IcmpHeader {
    type Error = IcmpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < ICMP_HEADER_LEN {
            return Err(IcmpError::TooShort { len: data.len() });
        }
        if checksum16(data) != 0 {
            return Err(IcmpError::InvalidChecksum);
        }
        Ok(Self {
            type_value: data[0],
            code: data[1],
            checksum: u16::from_be_bytes([data[2], data[3]]),
            dependent: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IcmpError {
    #[error("ICMP message is too short: {len} bytes")]
    TooShort { len: usize },
    #[error("invalid ICMP checksum")]
    InvalidChecksum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct IcmpPacket<'a> {
    #[getset(get_copy = "pub")]
    source: Ipv4Addr,
    #[getset(get_copy = "pub")]
    destination: Ipv4Addr,
    #[getset(get_copy = "pub")]
    header: IcmpHeader,
    #[getset(get = "pub")]
    payload: &'a [u8],
    data: &'a [u8],
}

impl<'a> IcmpPacket<'a> {
    pub fn from_ipv4(packet: Ipv4Packet<'a>) -> Result<Self, IcmpError> {
        let header = IcmpHeader::try_from(packet.payload())?;
        Ok(Self {
            source: packet.header().source(),
            destination: packet.header().destination(),
            header,
            payload: &packet.payload()[ICMP_HEADER_LEN..],
            data: packet.payload(),
        })
    }

    pub fn input(&self) {
        debug!(
            "{} => {}, len={}",
            self.source,
            self.destination,
            self.payload.len() + ICMP_HEADER_LEN
        );
        match IcmpType::try_from(self.header.type_value()) {
            Ok(kind) => debug!("type: {} ({})", self.header.type_value(), kind),
            Err(_) => debug!("type: {} (Unknown)", self.header.type_value()),
        }
        debug!("code: {}", self.header.code());
        debug!("sum: 0x{:04x}", self.header.checksum());
        match IcmpType::try_from(self.header.type_value()) {
            Ok(IcmpType::Echo | IcmpType::EchoReply) => {
                debug!("id: {}", self.header.dependent() >> 16);
                debug!("seq: {}", self.header.dependent() & 0xffff);
            }
            Ok(IcmpType::DestinationUnreachable) => {
                debug!("unused: {}", self.header.dependent());
            }
            _ => debug!("dep: 0x{:08x}", self.header.dependent()),
        }
        debugdump(self.data);
    }
}
