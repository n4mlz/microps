use getset::CopyGetters;
use thiserror::Error;

use super::{IP_HEADER_LEN, Ipv4Addr, VERSION};
use crate::protocol::checksum16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct Ipv4Header {
    #[getset(get_copy = "pub")]
    version: u8,
    #[getset(get_copy = "pub")]
    tos: u8,
    #[getset(get_copy = "pub")]
    id: u16,
    #[getset(get_copy = "pub")]
    flags: u8,
    #[getset(get_copy = "pub")]
    fragment_offset: u16,
    #[getset(get_copy = "pub")]
    ttl: u8,
    #[getset(get_copy = "pub")]
    protocol: u8,
    #[getset(get_copy = "pub")]
    checksum: Option<u16>,
    #[getset(get_copy = "pub")]
    source: Ipv4Addr,
    #[getset(get_copy = "pub")]
    destination: Ipv4Addr,
}

impl TryFrom<&[u8]> for Ipv4Header {
    type Error = Ipv4Error;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        const FRAGMENT_OFFSET_MASK: u16 = 0x1fff;

        if data.len() < IP_HEADER_LEN {
            return Err(Ipv4Error::TooShort { len: data.len() });
        }

        let version = data[0] >> 4;
        if version != VERSION {
            return Err(Ipv4Error::InvalidVersion { version });
        }

        let header_len = usize::from(data[0] & 0x0f) * 4;
        if header_len != IP_HEADER_LEN {
            return Err(Ipv4Error::InvalidHeaderLength { header_len });
        }
        if checksum16(&data[..IP_HEADER_LEN]) != 0 {
            return Err(Ipv4Error::InvalidChecksum);
        }

        let flags_and_offset = u16::from_be_bytes([data[6], data[7]]);
        Ok(Self {
            version,
            tos: data[1],
            id: u16::from_be_bytes([data[4], data[5]]),
            flags: (flags_and_offset >> 13) as u8,
            fragment_offset: flags_and_offset & FRAGMENT_OFFSET_MASK,
            ttl: data[8],
            protocol: data[9],
            checksum: Some(u16::from_be_bytes([data[10], data[11]])),
            source: Ipv4Addr::new([data[12], data[13], data[14], data[15]]),
            destination: Ipv4Addr::new([data[16], data[17], data[18], data[19]]),
        })
    }
}

impl Ipv4Header {
    pub fn new(protocol: u8, id: u16, source: Ipv4Addr, destination: Ipv4Addr) -> Self {
        Self {
            version: VERSION,
            tos: 0,
            id,
            flags: 0,
            fragment_offset: 0,
            ttl: 255,
            protocol,
            checksum: None,
            source,
            destination,
        }
    }

    pub fn to_bytes(&self, total_len: u16) -> [u8; IP_HEADER_LEN] {
        let flags_and_offset = (u16::from(self.flags) << 13) | self.fragment_offset;
        let mut data = [0; IP_HEADER_LEN];
        data[0] = self.version << 4 | 5;
        data[1] = self.tos;
        data[2..4].copy_from_slice(&total_len.to_be_bytes());
        data[4..6].copy_from_slice(&self.id.to_be_bytes());
        data[6..8].copy_from_slice(&flags_and_offset.to_be_bytes());
        data[8] = self.ttl;
        data[9] = self.protocol;
        data[10..12].copy_from_slice(&self.checksum.unwrap_or(0).to_be_bytes());
        data[12..16].copy_from_slice(self.source.as_bytes());
        data[16..20].copy_from_slice(self.destination.as_bytes());
        if self.checksum.is_none() {
            let checksum = checksum16(&data);
            data[10..12].copy_from_slice(&checksum.to_be_bytes());
        }
        data
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Ipv4Error {
    #[error("IPv4 packet is too short: {len} bytes")]
    TooShort { len: usize },
    #[error("unsupported IPv4 version: {version}")]
    InvalidVersion { version: u8 },
    #[error("unsupported IPv4 header length: {header_len} bytes")]
    InvalidHeaderLength { header_len: usize },
    #[error("invalid IPv4 header checksum")]
    InvalidChecksum,
    #[error("IPv4 total length is too small: {total_len} bytes")]
    TotalLengthTooSmall { total_len: usize },
    #[error("IPv4 packet is truncated: {len} < {total_len} bytes")]
    TotalTruncated { len: usize, total_len: usize },
    #[error("IPv4 fragmentation is not supported")]
    Fragmented,
    #[error("IPv4 payload is too large: {len} bytes")]
    PayloadTooLarge { len: usize },
}
