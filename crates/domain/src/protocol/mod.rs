use crate::DeviceMeta;

mod ipv4;

pub use ipv4::Ipv4;

/// Static contract for a protocol implementation.
pub trait Protocol {
    const TYPE: u16;

    fn input(meta: &DeviceMeta, data: &[u8], dst: Option<&[u8]>);
}

/// Protocol dispatcher and future extension point.
#[derive(Debug, Default, Clone, Copy)]
pub struct Protocols;

impl Protocols {
    /// Dispatch a received frame to the matching protocol implementation.
    pub fn input(kind: u16, meta: &DeviceMeta, data: &[u8], dst: Option<&[u8]>) -> bool {
        match kind {
            Ipv4::TYPE => {
                Ipv4::input(meta, data, dst);
                true
            }
            _ => false,
        }
    }
}
