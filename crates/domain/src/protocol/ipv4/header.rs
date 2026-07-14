use getset::CopyGetters;
use thiserror::Error;

use super::{HEADER_LEN, Ipv4Addr, VERSION};

/// The fixed-size IPv4 base header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct Ipv4Header {
    #[getset(get_copy = "pub")]
    version: u8,
    #[getset(get_copy = "pub")]
    tos: u8,
    #[getset(get_copy = "pub")]
    total_len: u16,
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
    checksum: u16,
    #[getset(get_copy = "pub")]
    source: Ipv4Addr,
    #[getset(get_copy = "pub")]
    destination: Ipv4Addr,
}

impl TryFrom<&[u8]> for Ipv4Header {
    type Error = Ipv4Error;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        const FRAGMENT_OFFSET_MASK: u16 = 0x1fff;

        if data.len() < HEADER_LEN {
            return Err(Ipv4Error::TooShort { len: data.len() });
        }

        let version = data[0] >> 4;
        if version != VERSION {
            return Err(Ipv4Error::InvalidVersion { version });
        }

        let header_len = usize::from(data[0] & 0x0f) * 4;
        if header_len != HEADER_LEN {
            return Err(Ipv4Error::InvalidHeaderLength { header_len });
        }
        if checksum16(&data[..HEADER_LEN]) != 0 {
            return Err(Ipv4Error::InvalidChecksum);
        }

        let flags_and_offset = u16::from_be_bytes([data[6], data[7]]);
        Ok(Self {
            version,
            tos: data[1],
            total_len: u16::from_be_bytes([data[2], data[3]]),
            id: u16::from_be_bytes([data[4], data[5]]),
            flags: (flags_and_offset >> 13) as u8,
            fragment_offset: flags_and_offset & FRAGMENT_OFFSET_MASK,
            ttl: data[8],
            protocol: data[9],
            checksum: u16::from_be_bytes([data[10], data[11]]),
            source: Ipv4Addr::new([data[12], data[13], data[14], data[15]]),
            destination: Ipv4Addr::new([data[16], data[17], data[18], data[19]]),
        })
    }
}

impl From<Ipv4Header> for [u8; HEADER_LEN] {
    fn from(header: Ipv4Header) -> Self {
        let flags_and_offset = (u16::from(header.flags) << 13) | header.fragment_offset;
        let mut data = [0; HEADER_LEN];
        data[0] = header.version << 4 | 5;
        data[1] = header.tos;
        data[2..4].copy_from_slice(&header.total_len.to_be_bytes());
        data[4..6].copy_from_slice(&header.id.to_be_bytes());
        data[6..8].copy_from_slice(&flags_and_offset.to_be_bytes());
        data[8] = header.ttl;
        data[9] = header.protocol;
        data[10..12].copy_from_slice(&header.checksum.to_be_bytes());
        data[12..16].copy_from_slice(&header.source.octets());
        data[16..20].copy_from_slice(&header.destination.octets());
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
}

fn checksum16(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let (chunks, remainder) = data.as_chunks::<2>();
    for chunk in chunks {
        sum += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
        sum = (sum & 0xffff) + (sum >> 16);
    }
    if let Some(byte) = remainder.first() {
        sum += u32::from(*byte) << 8;
        sum = (sum & 0xffff) + (sum >> 16);
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}
