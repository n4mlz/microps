use crate::{DeviceMeta, InterfaceRegistry};

mod ipv4;

pub use ipv4::{
    Ipv4, Ipv4Addr, Ipv4AddrParseError, Ipv4Error, Ipv4Header, Ipv4Interface, Ipv4Packet,
};

/// Static contract for a protocol implementation.
pub trait Protocol {
    const TYPE: u16;

    fn input(meta: &DeviceMeta, data: &[u8], interfaces: &InterfaceRegistry);
}

/// Protocol dispatcher and future extension point.
#[derive(Debug, Default, Clone, Copy)]
pub struct Protocols;

impl Protocols {
    /// Dispatch a received frame to the matching protocol implementation.
    pub fn input(kind: u16, meta: &DeviceMeta, data: &[u8], interfaces: &InterfaceRegistry) {
        if kind == Ipv4::TYPE {
            Ipv4::input(meta, data, interfaces);
        }
    }
}
