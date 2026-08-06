use alloc::vec::Vec;

use getset::{CopyGetters, Getters};

use super::{TCP_HEADER_LEN, TcpError, TcpFlags, TcpHeader};
use crate::protocol::{
    IPV4_PSEUDO_HEADER_LEN, Ipv4Endpoint, Ipv4Packet, Ipv4Protocol, Ipv4PseudoHeader, checksum16,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct TcpPacket<'a> {
    #[getset(get_copy = "pub")]
    src: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    dst: Ipv4Endpoint,
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
    pub fn build(
        src: Ipv4Endpoint,
        dst: Ipv4Endpoint,
        seq: u32,
        ack: u32,
        flags: TcpFlags,
        window_size: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, TcpError> {
        let length = TCP_HEADER_LEN
            .checked_add(payload.len())
            .filter(|length| *length <= usize::from(u16::MAX))
            .ok_or(TcpError::PayloadTooLarge { len: payload.len() })?;
        let mut data = Vec::with_capacity(length);
        data.extend_from_slice(
            &TcpHeader::new(src.port(), dst.port(), seq, ack, flags, window_size, 0).to_bytes(),
        );
        data.extend_from_slice(payload);

        let pseudo_header = Ipv4PseudoHeader::new(
            src.address(),
            dst.address(),
            Ipv4Protocol::Tcp,
            length as u16,
        );
        let mut checksum_data = Vec::with_capacity(IPV4_PSEUDO_HEADER_LEN + data.len());
        checksum_data.extend_from_slice(&pseudo_header.to_bytes());
        checksum_data.extend_from_slice(&data);
        data[16..18].copy_from_slice(&checksum16(&checksum_data).to_be_bytes());
        Ok(data)
    }

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

        let pseudo_header = Ipv4PseudoHeader::new(
            packet.header().src(),
            packet.header().dst(),
            Ipv4Protocol::Tcp,
            data.len() as u16,
        );
        let mut checksum_data = Vec::with_capacity(IPV4_PSEUDO_HEADER_LEN + data.len());
        checksum_data.extend_from_slice(&pseudo_header.to_bytes());
        checksum_data.extend_from_slice(data);
        if checksum16(&checksum_data) != 0 {
            return Err(TcpError::InvalidChecksum);
        }

        Ok(Self {
            src: Ipv4Endpoint::new(packet.header().src(), header.src_port()),
            dst: Ipv4Endpoint::new(packet.header().dst(), header.dst_port()),
            header,
            options: &data[TCP_HEADER_LEN..header_len],
            payload: &data[header_len..],
            data,
        })
    }
}
