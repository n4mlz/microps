use alloc::vec::Vec;

use getset::{CopyGetters, Getters};

use super::{UDP_HEADER_LEN, UDP_PSEUDO_HEADER_LEN, UdpError, UdpHeader, UdpPseudoHeader};
use crate::protocol::{Ipv4Endpoint, Ipv4Packet, checksum16};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct UdpPacket<'a> {
    #[getset(get_copy = "pub")]
    src: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    dest: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    header: UdpHeader,
    #[getset(get = "pub")]
    payload: &'a [u8],
    #[getset(get = "pub")]
    data: &'a [u8],
}

impl<'a> UdpPacket<'a> {
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
            let pseudo_header =
                UdpPseudoHeader::new(packet.header().src(), packet.header().dest(), length as u16);
            let mut checksum_data = Vec::with_capacity(UDP_PSEUDO_HEADER_LEN + data.len());
            checksum_data.extend_from_slice(&pseudo_header.to_bytes());
            checksum_data.extend_from_slice(data);
            if checksum16(&checksum_data) != 0 {
                return Err(UdpError::InvalidChecksum);
            }
        }

        Ok(Self {
            src: Ipv4Endpoint::new(packet.header().src(), header.src_port()),
            dest: Ipv4Endpoint::new(packet.header().dest(), header.dest_port()),
            header,
            payload: &data[UDP_HEADER_LEN..],
            data,
        })
    }
}
