use getset::CopyGetters;
use thiserror::Error;

use super::UDP_HEADER_LEN;
use crate::protocol::{Ipv4Addr, Ipv4Protocol};

pub(super) const UDP_PSEUDO_HEADER_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UdpPseudoHeader {
    src: Ipv4Addr,
    dest: Ipv4Addr,
    length: u16,
}

impl UdpPseudoHeader {
    pub(super) const fn new(src: Ipv4Addr, dest: Ipv4Addr, length: u16) -> Self {
        Self { src, dest, length }
    }

    pub(super) fn to_bytes(self) -> [u8; UDP_PSEUDO_HEADER_LEN] {
        let mut bytes = [0; UDP_PSEUDO_HEADER_LEN];
        bytes[..4].copy_from_slice(self.src.as_bytes());
        bytes[4..8].copy_from_slice(self.dest.as_bytes());
        bytes[9] = Ipv4Protocol::Udp as u8;
        bytes[10..].copy_from_slice(&self.length.to_be_bytes());
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct UdpHeader {
    #[getset(get_copy = "pub")]
    src_port: u16,
    #[getset(get_copy = "pub")]
    dest_port: u16,
    #[getset(get_copy = "pub")]
    length: u16,
    #[getset(get_copy = "pub")]
    checksum: u16,
}

impl TryFrom<&[u8]> for UdpHeader {
    type Error = UdpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < UDP_HEADER_LEN {
            return Err(UdpError::TooShort { len: data.len() });
        }
        Ok(Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dest_port: u16::from_be_bytes([data[2], data[3]]),
            length: u16::from_be_bytes([data[4], data[5]]),
            checksum: u16::from_be_bytes([data[6], data[7]]),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UdpError {
    #[error("UDP datagram is too short: {len} bytes")]
    TooShort { len: usize },
    #[error("UDP length is too small: {length} bytes")]
    LengthTooSmall { length: usize },
    #[error("UDP datagram is truncated: {len} < {length}")]
    LengthTruncated { len: usize, length: usize },
    #[error("invalid UDP checksum")]
    InvalidChecksum,
}
