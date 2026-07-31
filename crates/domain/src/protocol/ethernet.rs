use alloc::vec::Vec;
use core::{fmt, str::FromStr};

use getset::{CopyGetters, Getters};
use thiserror::Error;

pub const HEADER_LEN: usize = 14;
pub const FRAME_LEN_MIN: usize = 60;
pub const FRAME_LEN_MAX: usize = 1514;
pub const PAYLOAD_LEN_MIN: usize = FRAME_LEN_MIN - HEADER_LEN;
pub const PAYLOAD_LEN_MAX: usize = FRAME_LEN_MAX - HEADER_LEN;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EtherType {
    Ipv4 = 0x0800,
    Arp = 0x0806,
    Ipv6 = 0x86dd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, CopyGetters)]
pub struct MacAddr {
    #[getset(get_copy = "pub")]
    bytes: [u8; 6],
}

impl MacAddr {
    pub const ANY: Self = Self { bytes: [0; 6] };
    pub const BROADCAST: Self = Self { bytes: [0xff; 6] };

    pub const fn new(bytes: [u8; 6]) -> Self {
        Self { bytes }
    }
}

impl From<[u8; 6]> for MacAddr {
    fn from(bytes: [u8; 6]) -> Self {
        Self::new(bytes)
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.bytes.iter().enumerate() {
            if index != 0 {
                f.write_str(":")?;
            }
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for MacAddr {
    type Err = MacAddrParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0; 6];
        let mut parts = value.split(':');
        for byte in &mut bytes {
            let part = parts.next().ok_or(MacAddrParseError)?;
            if part.len() != 2 {
                return Err(MacAddrParseError);
            }
            *byte = u8::from_str_radix(part, 16).map_err(|_| MacAddrParseError)?;
        }
        if parts.next().is_some() {
            return Err(MacAddrParseError);
        }
        Ok(Self::new(bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("invalid MAC address")]
pub struct MacAddrParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct EthernetHeader {
    #[getset(get_copy = "pub")]
    destination: MacAddr,
    #[getset(get_copy = "pub")]
    source: MacAddr,
    #[getset(get_copy = "pub")]
    ether_type: u16,
}

impl EthernetHeader {
    pub const fn new(destination: MacAddr, source: MacAddr, ether_type: u16) -> Self {
        Self {
            destination,
            source,
            ether_type,
        }
    }

    fn bytes(self) -> [u8; HEADER_LEN] {
        let mut bytes = [0; HEADER_LEN];
        bytes[..6].copy_from_slice(&self.destination.bytes());
        bytes[6..12].copy_from_slice(&self.source.bytes());
        bytes[12..].copy_from_slice(&self.ether_type.to_be_bytes());
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters, Getters)]
pub struct EthernetFrame<'a> {
    #[getset(get_copy = "pub")]
    header: EthernetHeader,
    #[getset(get = "pub")]
    payload: &'a [u8],
}

impl EthernetFrame<'_> {
    pub fn build(
        source: MacAddr,
        destination: MacAddr,
        ether_type: EtherType,
        payload: &[u8],
    ) -> Result<Vec<u8>, EthernetError> {
        if payload.len() > PAYLOAD_LEN_MAX {
            return Err(EthernetError::PayloadTooLarge { len: payload.len() });
        }
        let header = EthernetHeader::new(destination, source, ether_type as u16);
        let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
        frame.extend_from_slice(&header.bytes());
        frame.extend_from_slice(payload);
        Ok(frame)
    }
}

impl<'a> TryFrom<&'a [u8]> for EthernetFrame<'a> {
    type Error = EthernetError;

    fn try_from(frame: &'a [u8]) -> Result<Self, Self::Error> {
        if frame.len() < HEADER_LEN {
            return Err(EthernetError::TooShort { len: frame.len() });
        }
        Ok(Self {
            header: EthernetHeader::new(
                MacAddr::new(frame[..6].try_into().unwrap()),
                MacAddr::new(frame[6..12].try_into().unwrap()),
                u16::from_be_bytes(frame[12..14].try_into().unwrap()),
            ),
            payload: &frame[HEADER_LEN..],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EthernetError {
    #[error("Ethernet frame is too short: {len} bytes")]
    TooShort { len: usize },
    #[error("Ethernet payload is too large: {len} bytes")]
    PayloadTooLarge { len: usize },
}
