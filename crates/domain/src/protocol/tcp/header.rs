use bitflags::bitflags;
use getset::CopyGetters;
use thiserror::Error;

use super::TCP_HEADER_LEN;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TcpFlags: u8 {
        const FIN = 0x01;
        const SYN = 0x02;
        const RST = 0x04;
        const PSH = 0x08;
        const ACK = 0x10;
        const URG = 0x20;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct TcpHeader {
    #[getset(get_copy = "pub")]
    src_port: u16,
    #[getset(get_copy = "pub")]
    dest_port: u16,
    #[getset(get_copy = "pub")]
    sequence_number: u32,
    #[getset(get_copy = "pub")]
    acknowledgment_number: u32,
    #[getset(get_copy = "pub")]
    data_offset: u8,
    #[getset(get_copy = "pub")]
    flags: TcpFlags,
    #[getset(get_copy = "pub")]
    window_size: u16,
    #[getset(get_copy = "pub")]
    checksum: u16,
    #[getset(get_copy = "pub")]
    urgent_pointer: u16,
}

impl TcpHeader {
    pub fn header_len(&self) -> usize {
        usize::from(self.data_offset >> 4) * 4
    }
}

impl TryFrom<&[u8]> for TcpHeader {
    type Error = TcpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        if data.len() < TCP_HEADER_LEN {
            return Err(TcpError::TooShort { len: data.len() });
        }
        let data_offset = data[12];
        let header_len = usize::from(data_offset >> 4) * 4;
        if header_len < TCP_HEADER_LEN {
            return Err(TcpError::HeaderLengthTooSmall { header_len });
        }
        if data_offset & 0x0f != 0 {
            return Err(TcpError::InvalidHeaderLength { data_offset });
        }

        Ok(Self {
            src_port: u16::from_be_bytes([data[0], data[1]]),
            dest_port: u16::from_be_bytes([data[2], data[3]]),
            sequence_number: u32::from_be_bytes(data[4..8].try_into().unwrap()),
            acknowledgment_number: u32::from_be_bytes(data[8..12].try_into().unwrap()),
            data_offset,
            flags: TcpFlags::from_bits_truncate(data[13]),
            window_size: u16::from_be_bytes([data[14], data[15]]),
            checksum: u16::from_be_bytes([data[16], data[17]]),
            urgent_pointer: u16::from_be_bytes([data[18], data[19]]),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TcpError {
    #[error("TCP segment is too short: {len} bytes")]
    TooShort { len: usize },
    #[error("TCP header length is too small: {header_len} bytes")]
    HeaderLengthTooSmall { header_len: usize },
    #[error("invalid TCP data offset: 0x{data_offset:02x}")]
    InvalidHeaderLength { data_offset: u8 },
    #[error("TCP header is truncated: {len} < {header_len}")]
    HeaderTruncated { len: usize, header_len: usize },
    #[error("invalid TCP checksum")]
    InvalidChecksum,
}
