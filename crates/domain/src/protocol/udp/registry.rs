use alloc::{collections::VecDeque, vec::Vec};

use getset::{CopyGetters, Getters};
use slotmap::{SlotMap, new_key_type};
use thiserror::Error;

use crate::{
    Lock, Platform, WaitResult,
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
    pub fn new(remote: Ipv4Endpoint, payload: Vec<u8>) -> Self {
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
    #[error("wait interrupted")]
    Interrupted,
    #[error("UDP PCB does not exist")]
    NotFound,
    #[error("UDP endpoint is already in use")]
    AlreadyBound,
    #[error("no ephemeral UDP port is available")]
    NoEphemeralPort,
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
        let result = self
            .pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible")
            .remove(pcb)
            .map(|_| ())
            .ok_or(UdpPcbError::NotFound);
        self.pcbs.wake_all();
        result
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

    pub fn select(&self, local: Ipv4Endpoint) -> Option<UdpPcbKey> {
        self.pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible")
            .iter()
            .find(|(_, pcb)| Self::matches(pcb.local, local))
            .map(|(key, _)| key)
    }

    pub fn enqueue(&self, pcb: UdpPcbKey, datagram: ReceivedDatagram) -> Result<(), UdpPcbError> {
        self.pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible")
            .get_mut(pcb)
            .ok_or(UdpPcbError::NotFound)?
            .receive_queue
            .push_back(datagram);
        self.pcbs.wake_all();
        Ok(())
    }

    pub fn recv_from(
        &self,
        pcb: UdpPcbKey,
        buffer: &mut [u8],
    ) -> Result<(usize, Ipv4Endpoint), UdpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible");
        loop {
            let socket = pcbs.get_mut(pcb).ok_or(UdpPcbError::NotFound)?;
            if let Some(datagram) = socket.receive_queue.pop_front() {
                let length = buffer.len().min(datagram.payload().len());
                buffer[..length].copy_from_slice(&datagram.payload()[..length]);
                return Ok((length, datagram.remote()));
            }
            pcbs = match self.pcbs.wait(pcbs).expect("UDP PCB wait is infallible") {
                WaitResult::Notified(pcbs) => pcbs,
                WaitResult::Interrupted(_) => return Err(UdpPcbError::Interrupted),
            };
        }
    }

    pub fn assign_dynamic_port(&self, pcb: UdpPcbKey) -> Result<Ipv4Endpoint, UdpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("UDP PCB registry lock is infallible");
        let local = pcbs.get(pcb).ok_or(UdpPcbError::NotFound)?.local;
        if local.port() != 0 {
            return Ok(local);
        }
        for port in crate::protocol::DYNAMIC_PORT_MIN..=crate::protocol::DYNAMIC_PORT_MAX {
            let candidate = Ipv4Endpoint::new(local.address(), port);
            if !pcbs
                .iter()
                .any(|(key, socket)| key != pcb && Self::matches(socket.local, candidate))
            {
                pcbs[pcb].local = candidate;
                return Ok(candidate);
            }
        }
        Err(UdpPcbError::NoEphemeralPort)
    }

    pub fn interrupt_all(&self) {
        self.pcbs.interrupt_all();
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
