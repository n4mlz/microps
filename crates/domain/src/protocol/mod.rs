mod arp;
mod ethernet;
mod icmp;
mod ipv4;
mod udp;

pub use arp::*;
pub use ethernet::*;
pub use icmp::*;
pub use ipv4::*;
pub use udp::*;

pub(crate) fn checksum16(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    let (chunks, remainder) = data.as_chunks::<2>();
    for chunk in chunks {
        sum += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
        sum = (sum & 0xffff) + (sum >> 16);
    }
    if let Some(byte) = remainder.first() {
        sum += u32::from(*byte) << 8;
        sum = (sum & 0xffff) + (sum >> 16);
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}
