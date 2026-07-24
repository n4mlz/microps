mod addr;
mod header;
mod packet;

pub use addr::{Ipv4Addr, Ipv4AddrParseError};
pub use header::{Ipv4Error, Ipv4Header};
pub use packet::Ipv4Packet;

use super::Protocol;
use crate::{DeviceMeta, debug, error};

/// IPv4 version carried in the high four bits of the first header byte.
const VERSION: u8 = 4;

/// Length of the IPv4 base header in bytes; options are not supported yet.
const HEADER_LEN: usize = 20;

/// Zero-sized IPv4 protocol implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Ipv4;

impl Protocol for Ipv4 {
    const TYPE: u16 = 0x0800;

    fn input(meta: &DeviceMeta, data: &[u8], _dst: Option<&[u8]>) {
        debug!("dev={}, len={}", meta.name(), data.len());
        let packet = match Ipv4Packet::try_from(data) {
            Ok(packet) => packet,
            Err(error) => {
                error!("{error}");
                return;
            }
        };
        let header = packet.header();

        debug!(
            "vhl: 0x{:02x} [v: {}, hl: 5 (20)]",
            data[0],
            header.version()
        );
        debug!("tos: 0x{:02x}", header.tos());
        debug!(
            "total: {} (payload: {})",
            packet.total_len(),
            packet.payload().len()
        );
        debug!("id: {}", header.id());
        debug!(
            "offset: 0x{:04x} [flags={}, offset={}]",
            (u16::from(header.flags()) << 13) | header.fragment_offset(),
            header.flags(),
            header.fragment_offset()
        );
        debug!("ttl: {}", header.ttl());
        debug!("protocol: {}", header.protocol());
        debug!("sum: 0x{:04x}", header.checksum());
        debug!("src: {}", header.source());
        debug!("dst: {}", header.destination());
    }
}
