use alloc::vec::Vec;

use getset::{CopyGetters, Getters};

use super::{UDP_HEADER_LEN, UdpError, UdpHeader};
use crate::protocol::{
    IPV4_PSEUDO_HEADER_LEN, Ipv4Endpoint, Ipv4Packet, Ipv4Protocol, Ipv4PseudoHeader, checksum16,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct UdpPacket<'a> {
    #[getset(get_copy = "pub")]
    src: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    dst: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    header: UdpHeader,
    #[getset(get = "pub")]
    payload: &'a [u8],
    #[getset(get = "pub")]
    data: &'a [u8],
}

impl<'a> UdpPacket<'a> {
    pub fn build(
        src: Ipv4Endpoint,
        dst: Ipv4Endpoint,
        payload: &[u8],
    ) -> Result<Vec<u8>, UdpError> {
        let length = UDP_HEADER_LEN
            .checked_add(payload.len())
            .filter(|length| *length <= usize::from(u16::MAX))
            .ok_or(UdpError::PayloadTooLarge { len: payload.len() })?;
        let pseudo_header = Ipv4PseudoHeader::new(
            src.address(),
            dst.address(),
            Ipv4Protocol::Udp,
            length as u16,
        );
        let mut data = Vec::with_capacity(length);
        data.extend_from_slice(
            &UdpHeader::new(src.port(), dst.port(), length as u16, 0).to_bytes(),
        );
        data.extend_from_slice(payload);
        let mut checksum_data = Vec::with_capacity(IPV4_PSEUDO_HEADER_LEN + length);
        checksum_data.extend_from_slice(&pseudo_header.to_bytes());
        checksum_data.extend_from_slice(&data);
        let checksum = match checksum16(&checksum_data) {
            0 => u16::MAX,
            checksum => checksum,
        };
        data[6..8].copy_from_slice(&checksum.to_be_bytes());
        Ok(data)
    }

    pub fn from_ipv4(packet: Ipv4Packet<'a>) -> Result<Self, UdpError> {
        let data = packet.payload();
        let header = UdpHeader::try_from(data)?;
        let length = usize::from(header.length());
        if length < UDP_HEADER_LEN {
            return Err(UdpError::LengthTooSmall { length });
        }
        if data.len() < length {
            return Err(UdpError::LengthTruncated {
                len: data.len(),
                length,
            });
        }

        let data = &data[..length];
        if header.checksum() != 0 {
            let pseudo_header = Ipv4PseudoHeader::new(
                packet.header().src(),
                packet.header().dst(),
                Ipv4Protocol::Udp,
                length as u16,
            );
            let mut checksum_data = Vec::with_capacity(IPV4_PSEUDO_HEADER_LEN + data.len());
            checksum_data.extend_from_slice(&pseudo_header.to_bytes());
            checksum_data.extend_from_slice(data);
            if checksum16(&checksum_data) != 0 {
                return Err(UdpError::InvalidChecksum);
            }
        }

        Ok(Self {
            src: Ipv4Endpoint::new(packet.header().src(), header.src_port()),
            dst: Ipv4Endpoint::new(packet.header().dst(), header.dst_port()),
            header,
            payload: &data[UDP_HEADER_LEN..],
            data,
        })
    }
}
