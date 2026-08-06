use alloc::{collections::VecDeque, vec::Vec};

use getset::{CopyGetters, Getters};
use slotmap::{SlotMap, new_key_type};
use thiserror::Error;

use crate::{
    Lock, Platform,
    protocol::{Ipv4Addr, Ipv4Endpoint},
};

new_key_type! {
    /// Stable key for a UDP PCB owned by a [`UdpPcbRegistry`].
    pub struct UdpPcbKey;
}

#[derive(Debug, Clone, PartialEq, Eq, Getters, CopyGetters)]
pub struct ReceivedDatagram {
    #[getset(get_copy = "pub")]
    remote: Ipv4Endpoint,
    #[getset(get = "pub")]
    payload: Vec<u8>,
}

impl ReceivedDatagram {
    pub(super) fn new(remote: Ipv4Endpoint, payload: Vec<u8>) -> Self {
        Self { remote, payload }
    }
}

#[derive(Debug)]
struct UdpPcb {
    local: Ipv4Endpoint,
    receive_queue: VecDeque<ReceivedDatagram>,
}

#[derive(Debug)]
pub struct UdpPcbRegistry<P: Platform> {
    pcbs: P::Mutex<SlotMap<UdpPcbKey, UdpPcb>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum UdpPcbError {
    #[error("UDP PCB does not exist")]
    NotFound,
    #[error("UDP endpoint is already in use")]
    AlreadyBound,
}

impl<P: Platform> UdpPcbRegistry<P> {
    pub fn open(&self) -> UdpPcbKey {
        self.pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible")
            .insert(UdpPcb {
                local: Ipv4Endpoint::new(Ipv4Addr::ANY, 0),
                receive_queue: VecDeque::new(),
            })
    }

    pub fn close(&self, pcb: UdpPcbKey) -> Result<(), UdpPcbError> {
        self.pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible")
            .remove(pcb)
            .map(|_| ())
            .ok_or(UdpPcbError::NotFound)
    }

    pub fn bind(&self, pcb: UdpPcbKey, local: Ipv4Endpoint) -> Result<(), UdpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible");
        if !pcbs.contains_key(pcb) {
            return Err(UdpPcbError::NotFound);
        }
        if pcbs
            .iter()
            .any(|(key, other)| key != pcb && Self::matches(other.local, local))
        {
            return Err(UdpPcbError::AlreadyBound);
        }
        pcbs[pcb].local = local;
        Ok(())
    }

    pub(super) fn select(&self, local: Ipv4Endpoint) -> Option<UdpPcbKey> {
        self.pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible")
            .iter()
            .find(|(_, pcb)| Self::matches(pcb.local, local))
            .map(|(key, _)| key)
    }

    pub(super) fn enqueue(
        &self,
        pcb: UdpPcbKey,
        datagram: ReceivedDatagram,
    ) -> Result<(), UdpPcbError> {
        self.pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible")
            .get_mut(pcb)
            .ok_or(UdpPcbError::NotFound)?
            .receive_queue
            .push_back(datagram);
        Ok(())
    }

    fn matches(bound: Ipv4Endpoint, requested: Ipv4Endpoint) -> bool {
        bound.port() == requested.port()
            && (bound.address() == requested.address()
                || bound.address() == Ipv4Addr::ANY
                || requested.address() == Ipv4Addr::ANY)
    }
}

impl<P: Platform> Default for UdpPcbRegistry<P> {
    fn default() -> Self {
        Self {
            pcbs: P::Mutex::new(SlotMap::default()),
        }
    }
}
