use alloc::vec::Vec;
use core::fmt;

use getset::{CopyGetters, Getters};
use thiserror::Error;

use crate::{
    Platform, Random, debug, debugdump,
    protocol::{
        checksum16,
        ipv4::{Ipv4, Ipv4Addr, Ipv4Interface, Ipv4OutputError, Ipv4Packet, Ipv4Protocol},
    },
};

pub const ICMP_HEADER_LEN: usize = 8;

pub struct Icmp;

impl Icmp {
    /// The unused field value for ICMP messages that do not define it.
    pub const UNUSED: u32 = 0;
}

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

/// Codes defined for ICMP Destination Unreachable messages.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IcmpDestinationUnreachableCode {
    NetworkUnreachable = 0,
    HostUnreachable = 1,
    ProtocolUnreachable = 2,
    PortUnreachable = 3,
    FragmentationNeeded = 4,
    SourceRouteFailed = 5,
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
    src: Ipv4Addr,
    #[getset(get_copy = "pub")]
    dest: Ipv4Addr,
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
            src: packet.header().src(),
            dest: packet.header().dest(),
            header,
            payload: &packet.payload()[ICMP_HEADER_LEN..],
            data: packet.payload(),
        })
    }
}

impl Icmp {
    pub fn echo_reply<P: Platform + 'static, R: Random>(
        interface: &Ipv4Interface,
        request: &IcmpPacket<'_>,
    ) -> Result<usize, Ipv4OutputError<R::Error>> {
        Self::output::<P, R>(
            interface,
            IcmpType::EchoReply as u8,
            request.header.code(),
            request.header.dependent(),
            request.payload,
            interface.unicast(),
            request.src,
        )
    }

    pub fn destination_unreachable<P: Platform + 'static, R: Random>(
        interface: &Ipv4Interface,
        offending: &[u8],
        dest: Ipv4Addr,
    ) -> Result<usize, Ipv4OutputError<R::Error>> {
        Self::output::<P, R>(
            interface,
            IcmpType::DestinationUnreachable as u8,
            IcmpDestinationUnreachableCode::ProtocolUnreachable as u8,
            Self::UNUSED,
            offending,
            interface.unicast(),
            dest,
        )
    }

    pub fn output<P: Platform + 'static, R: Random>(
        interface: &Ipv4Interface,
        type_value: u8,
        code: u8,
        dependent: u32,
        payload: &[u8],
        src: Ipv4Addr,
        dest: Ipv4Addr,
    ) -> Result<usize, Ipv4OutputError<R::Error>> {
        let mut data = Vec::with_capacity(ICMP_HEADER_LEN + payload.len());
        data.extend_from_slice(&[type_value, code, 0, 0]);
        data.extend_from_slice(&dependent.to_be_bytes());
        data.extend_from_slice(payload);
        let checksum = checksum16(&data);
        data[2..4].copy_from_slice(&checksum.to_be_bytes());
        Ipv4::output::<P, R>(interface, Ipv4Protocol::Icmp as u8, &data, src, dest)
    }

    pub fn input<P: Platform + 'static, R: Random>(
        packet: Ipv4Packet<'_>,
        interface: &Ipv4Interface,
    ) -> Result<(), IcmpError> {
        let packet = IcmpPacket::from_ipv4(packet)?;
        debug!(
            "{} => {}, len={}",
            packet.src,
            packet.dest,
            packet.payload.len() + ICMP_HEADER_LEN
        );
        match IcmpType::try_from(packet.header.type_value()) {
            Ok(kind) => debug!("type: {} ({})", packet.header.type_value(), kind),
            Err(_) => debug!("type: {} (Unknown)", packet.header.type_value()),
        }
        debug!("code: {}", packet.header.code());
        debug!("sum: 0x{:04x}", packet.header.checksum());
        match IcmpType::try_from(packet.header.type_value()) {
            Ok(IcmpType::Echo | IcmpType::EchoReply) => {
                debug!("id: {}", packet.header.dependent() >> 16);
                debug!("seq: {}", packet.header.dependent() & 0xffff);
            }
            Ok(IcmpType::DestinationUnreachable) => debug!("unused: {}", packet.header.dependent()),
            _ => debug!("dep: 0x{:08x}", packet.header.dependent()),
        }
        debugdump(packet.data);
        if packet.header.type_value() == IcmpType::Echo as u8
            && let Err(error) = Self::echo_reply::<P, R>(interface, &packet)
        {
            crate::error!("{error}");
        }
        Ok(())
    }
}
