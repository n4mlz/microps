use alloc::vec::Vec;

use getset::{CopyGetters, Getters};

use super::{IP_HEADER_LEN, Ipv4Error, Ipv4Header};

/// An IPv4 packet with parsed header fields and a borrowed payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Getters, CopyGetters)]
pub struct Ipv4Packet<'a> {
    #[getset(get_copy = "pub")]
    header: Ipv4Header,
    #[getset(get_copy = "pub")]
    payload: &'a [u8],
    #[getset(get = "pub")]
    data: &'a [u8],
}

impl<'a> Ipv4Packet<'a> {
    pub fn packet_len(&self) -> usize {
        IP_HEADER_LEN + self.payload.len()
    }

    pub fn build(
        protocol: u8,
        payload: &[u8],
        id: u16,
        src: super::Ipv4Addr,
        dest: super::Ipv4Addr,
    ) -> Result<Vec<u8>, Ipv4Error> {
        let total_len = IP_HEADER_LEN
            .checked_add(payload.len())
            .ok_or(Ipv4Error::PayloadTooLarge { len: payload.len() })?;
        if total_len > usize::from(u16::MAX) {
            return Err(Ipv4Error::PayloadTooLarge { len: payload.len() });
        }

        let header = Ipv4Header::new(protocol, id, src, dest);
        let mut packet = Vec::with_capacity(total_len);
        packet.extend_from_slice(&header.to_bytes(total_len as u16));
        packet.extend_from_slice(payload);
        Ok(packet)
    }
}

impl<'a> TryFrom<&'a [u8]> for Ipv4Packet<'a> {
    type Error = Ipv4Error;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        let header = Ipv4Header::try_from(data)?;
        let total_len = usize::from(u16::from_be_bytes([data[2], data[3]]));
        if total_len < IP_HEADER_LEN {
            return Err(Ipv4Error::TotalLengthTooSmall { total_len });
        }
        if data.len() < total_len {
            return Err(Ipv4Error::TotalTruncated {
                len: data.len(),
                total_len,
            });
        }
        if header.flags() & 1 != 0 || header.fragment_offset() != 0 {
            return Err(Ipv4Error::Fragmented);
        }

        Ok(Self {
            header,
            payload: &data[IP_HEADER_LEN..total_len],
            data: &data[..total_len],
        })
    }
}
