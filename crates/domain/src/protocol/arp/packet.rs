use alloc::vec::Vec;

use getset::CopyGetters;

use super::{ArpHeader, ArpOperation, PACKET_LEN};
use crate::protocol::{EtherType, Ipv4Addr, MacAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct ArpPacket {
    #[getset(get_copy = "pub")]
    header: ArpHeader,
    #[getset(get_copy = "pub")]
    sender_hardware: MacAddr,
    #[getset(get_copy = "pub")]
    sender_protocol: Ipv4Addr,
    #[getset(get_copy = "pub")]
    target_hardware: MacAddr,
    #[getset(get_copy = "pub")]
    target_protocol: Ipv4Addr,
}

impl ArpPacket {
    pub fn build(
        operation: ArpOperation,
        sender_hardware: MacAddr,
        sender_protocol: Ipv4Addr,
        target_hardware: MacAddr,
        target_protocol: Ipv4Addr,
    ) -> Vec<u8> {
        let mut data = Vec::with_capacity(PACKET_LEN);
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&(EtherType::Ipv4 as u16).to_be_bytes());
        data.extend_from_slice(&[6, 4]);
        data.extend_from_slice(&(operation as u16).to_be_bytes());
        data.extend_from_slice(&sender_hardware.bytes());
        data.extend_from_slice(sender_protocol.as_bytes());
        data.extend_from_slice(&target_hardware.bytes());
        data.extend_from_slice(target_protocol.as_bytes());
        data
    }
}

impl TryFrom<&[u8]> for ArpPacket {
    type Error = super::ArpError;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        let header = ArpHeader::try_from(data)?;
        Ok(Self {
            header,
            sender_hardware: MacAddr::new(data[8..14].try_into().unwrap()),
            sender_protocol: Ipv4Addr::new(data[14..18].try_into().unwrap()),
            target_hardware: MacAddr::new(data[18..24].try_into().unwrap()),
            target_protocol: Ipv4Addr::new(data[24..28].try_into().unwrap()),
        })
    }
}
