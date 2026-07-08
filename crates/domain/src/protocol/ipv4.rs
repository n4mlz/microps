use super::Protocol;
use crate::{DeviceMeta, debug, debugdump};

/// Zero-sized IPv4 protocol implementation.
#[derive(Debug, Default, Clone, Copy)]
pub struct Ipv4;

impl Protocol for Ipv4 {
    const TYPE: u16 = 0x0800;

    fn input(meta: &DeviceMeta, data: &[u8], _dst: Option<&[u8]>) {
        debug!("dev={}, len={}", meta.name(), data.len());
        debugdump(data);
    }
}
