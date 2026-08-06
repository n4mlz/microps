use alloc::vec::Vec;

use getset::{CopyGetters, Getters};

use super::{TCP_HEADER_LEN, TcpError, TcpHeader};
use crate::protocol::{Ipv4Endpoint, Ipv4Packet, Ipv4Protocol, checksum16};

const TCP_PSEUDO_HEADER_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct TcpPacket<'a> {
    #[getset(get_copy = "pub")]
    src: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    dest: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    header: TcpHeader,
    #[getset(get = "pub")]
    options: &'a [u8],
    #[getset(get = "pub")]
    payload: &'a [u8],
    #[getset(get = "pub")]
    data: &'a [u8],
}

impl<'a> TcpPacket<'a> {
    pub fn from_ipv4(packet: Ipv4Packet<'a>) -> Result<Self, TcpError> {
        let data = packet.payload();
        let header = TcpHeader::try_from(data)?;
        let header_len = header.header_len();
        if data.len() < header_len {
            return Err(TcpError::HeaderTruncated {
                len: data.len(),
                header_len,
            });
        }

        let mut pseudo_header = [0; TCP_PSEUDO_HEADER_LEN];
        pseudo_header[..4].copy_from_slice(packet.header().src().as_bytes());
        pseudo_header[4..8].copy_from_slice(packet.header().dest().as_bytes());
        pseudo_header[9] = Ipv4Protocol::Tcp as u8;
        pseudo_header[10..].copy_from_slice(&(data.len() as u16).to_be_bytes());
        let mut checksum_data = Vec::with_capacity(pseudo_header.len() + data.len());
        checksum_data.extend_from_slice(&pseudo_header);
        checksum_data.extend_from_slice(data);
        if checksum16(&checksum_data) != 0 {
            return Err(TcpError::InvalidChecksum);
        }

        Ok(Self {
            src: Ipv4Endpoint::new(packet.header().src(), header.src_port()),
            dest: Ipv4Endpoint::new(packet.header().dest(), header.dest_port()),
            header,
            options: &data[TCP_HEADER_LEN..header_len],
            payload: &data[header_len..],
            data,
        })
    }
}
