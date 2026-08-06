use getset::CopyGetters;
use thiserror::Error;

use super::UDP_HEADER_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct UdpHeader {
    #[getset(get_copy = "pub")]
    src_port: u16,
    #[getset(get_copy = "pub")]
    dst_port: u16,
    #[getset(get_copy = "pub")]
    length: u16,
    #[getset(get_copy = "pub")]
    checksum: u16,
}

impl UdpHeader {
    pub const fn new(src_port: u16, dst_port: u16, length: u16, checksum: u16) -> Self {
        Self {
            src_port,
            dst_port,
            length,
            checksum,
        }
    }

    pub fn to_bytes(self) -> [u8; UDP_HEADER_LEN] {
        let mut bytes = [0; UDP_HEADER_LEN];
        bytes[..2].copy_from_slice(&self.src_port.to_be_bytes());
        bytes[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        bytes[4..6].copy_from_slice(&self.length.to_be_bytes());
        bytes[6..].copy_from_slice(&self.checksum.to_be_bytes());
        bytes
    }
}

impl TryFrom<&[u8]> for UdpHeader {
    type Error = UdpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < UDP_HEADER_LEN {
            return Err(UdpError::TooShort { len: data.len() });
        }
        Ok(Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dst_port: u16::from_be_bytes([data[2], data[3]]),
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
    #[error("UDP payload is too large: {len} bytes")]
    PayloadTooLarge { len: usize },
}
