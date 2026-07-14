use getset::CopyGetters;

use super::{HEADER_LEN, Ipv4Error, Ipv4Header};

/// An IPv4 packet with parsed header fields and a borrowed payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct Ipv4Packet<'a> {
    #[getset(get_copy = "pub")]
    header: Ipv4Header,
    #[getset(get_copy = "pub")]
    payload: &'a [u8],
}

impl Ipv4Packet<'_> {
    pub fn total_len(&self) -> usize {
        self.header.total_len() as usize
    }
}

impl<'a> TryFrom<&'a [u8]> for Ipv4Packet<'a> {
    type Error = Ipv4Error;

    fn try_from(data: &'a [u8]) -> Result<Self, Self::Error> {
        let header = Ipv4Header::try_from(data)?;
        let total_len = usize::from(u16::from_be_bytes([data[2], data[3]]));
        if total_len < HEADER_LEN {
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
            payload: &data[HEADER_LEN..total_len],
        })
    }
}
