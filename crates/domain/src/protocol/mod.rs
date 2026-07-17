use alloc::{collections::VecDeque, vec::Vec};

use crate::{DeviceMeta, InterfaceRegistry};

mod ethernet;
mod ipv4;

pub use ethernet::{
    ADDRESS_LEN, EthernetAddress, EthernetAddressParseError, EthernetError, EthernetFrame,
    HEADER_LEN, ethertype,
};
pub use ipv4::{
    Ipv4, Ipv4Addr, Ipv4AddrParseError, Ipv4Error, Ipv4Header, Ipv4Interface, Ipv4Packet,
};

/// Static contract for a protocol implementation.
pub trait Protocol {
    const TYPE: u16;

    fn input(meta: &DeviceMeta, data: &[u8], interfaces: &InterfaceRegistry);
}

/// Owned packet waiting for protocol-level processing.
#[derive(Debug)]
struct QueuedPacket {
    frame_type: u16,
    meta: DeviceMeta,
    data: Vec<u8>,
}

/// Input queue separating device interrupt handling from protocol processing.
#[derive(Debug, Default)]
pub struct ProtocolInputQueue {
    packets: VecDeque<QueuedPacket>,
}

const MAX_QUEUED_PACKETS: usize = 256;

impl ProtocolInputQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, frame_type: u16, meta: &DeviceMeta, data: &[u8]) -> bool {
        if frame_type != Ipv4::TYPE {
            return true;
        }
        if self.packets.len() >= MAX_QUEUED_PACKETS {
            return false;
        }
        self.packets.push_back(QueuedPacket {
            frame_type,
            meta: meta.clone(),
            data: data.to_vec(),
        });
        true
    }

    pub fn process(&mut self, interfaces: &InterfaceRegistry) {
        while let Some(packet) = self.packets.pop_front() {
            Protocols::input(packet.frame_type, &packet.meta, &packet.data, interfaces);
        }
    }
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
