use getset::CopyGetters;
use slotmap::{SlotMap, new_key_type};

use super::{Ipv4Endpoint, TcpState};
use crate::{Lock, Platform};

new_key_type! {
    pub struct TcpPcbKey;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct TcpPcb {
    #[getset(get_copy = "pub")]
    state: TcpState,
    #[getset(get_copy = "pub")]
    local: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    remote: Ipv4Endpoint,
    #[getset(get_copy = "pub")]
    iss: u32,
    #[getset(get_copy = "pub")]
    snd_nxt: u32,
    #[getset(get_copy = "pub")]
    snd_una: u32,
    #[getset(get_copy = "pub")]
    rcv_nxt: u32,
    #[getset(get_copy = "pub")]
    rcv_wnd: u16,
}

impl TcpPcb {
    pub fn new() -> Self {
        Self {
            state: TcpState::Closed,
            local: Ipv4Endpoint::new(crate::protocol::Ipv4Addr::ANY, 0),
            remote: Ipv4Endpoint::new(crate::protocol::Ipv4Addr::ANY, 0),
            iss: 0,
            snd_nxt: 0,
            snd_una: 0,
            rcv_nxt: 0,
            rcv_wnd: 0,
        }
    }

    pub fn listen(&mut self, local: Ipv4Endpoint, remote: Ipv4Endpoint) {
        self.local = local;
        self.remote = remote;
        self.state = TcpState::Listen;
    }

    pub fn accept_syn(&mut self, local: Ipv4Endpoint, remote: Ipv4Endpoint, seq: u32, iss: u32) {
        self.local = local;
        self.remote = remote;
        self.iss = iss;
        self.rcv_nxt = seq.wrapping_add(1);
        self.rcv_wnd = u16::MAX;
        self.snd_nxt = iss.wrapping_add(1);
        self.snd_una = iss;
        self.state = TcpState::SynReceived;
    }

    pub fn accept_ack(&mut self, ack: u32) -> bool {
        if self.state != TcpState::SynReceived {
            return false;
        }
        if self.snd_una <= ack && ack <= self.snd_nxt {
            self.state = TcpState::Established;
            true
        } else {
            false
        }
    }
}

impl Default for TcpPcb {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct TcpPcbRegistry<P: Platform> {
    pcbs: P::Mutex<SlotMap<TcpPcbKey, TcpPcb>>,
}

impl<P: Platform> TcpPcbRegistry<P> {
    pub fn open(&self) -> TcpPcbKey {
        self.pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible")
            .insert(TcpPcb::new())
    }

    pub fn close(&self, pcb: TcpPcbKey) -> bool {
        let result = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible")
            .remove(pcb)
            .is_some();
        self.pcbs.wake_all();
        result
    }

    pub fn get(&self, pcb: TcpPcbKey) -> Result<TcpPcb, TcpPcbError> {
        self.pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible")
            .get(pcb)
            .copied()
            .ok_or(TcpPcbError::NotFound)
    }

    pub fn replace(&self, key: TcpPcbKey, pcb: TcpPcb) -> Result<(), TcpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let entry = pcbs.get_mut(key).ok_or(TcpPcbError::NotFound)?;
        *entry = pcb;
        self.pcbs.wake_all();
        Ok(())
    }

    pub fn endpoint_in_use(
        &self,
        excluded: TcpPcbKey,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) -> bool {
        self.pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible")
            .iter()
            .any(|(key, pcb)| {
                key != excluded
                    && pcb.state() != TcpState::Closed
                    && Self::matches(*pcb, local, remote)
            })
    }

    pub fn wait_for_state_change(
        &self,
        pcb: TcpPcbKey,
        state: TcpState,
    ) -> Result<TcpState, TcpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        loop {
            let current = pcbs.get(pcb).ok_or(TcpPcbError::NotFound)?.state();
            if current != state {
                return Ok(current);
            }
            pcbs = self.pcbs.wait(pcbs).expect("TCP PCB wait is infallible");
        }
    }

    pub fn select(
        &self,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) -> Option<(TcpPcbKey, TcpState)> {
        self.pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible")
            .iter()
            .find(|(_, pcb)| Self::matches(**pcb, local, remote))
            .map(|(key, pcb)| (key, pcb.state()))
    }

    fn matches(pcb: TcpPcb, local: Ipv4Endpoint, remote: Ipv4Endpoint) -> bool {
        (pcb.local().port() == local.port())
            && (pcb.local().address() == local.address()
                || pcb.local().address() == crate::protocol::Ipv4Addr::ANY)
            && ((pcb.remote() == remote)
                || (pcb.remote().address() == crate::protocol::Ipv4Addr::ANY
                    && pcb.remote().port() == 0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TcpPcbError {
    #[error("TCP PCB does not exist")]
    NotFound,
    #[error("TCP endpoint is already in use")]
    AlreadyBound,
}

impl<P: Platform> Default for TcpPcbRegistry<P> {
    fn default() -> Self {
        Self {
            pcbs: P::Mutex::new(SlotMap::default()),
        }
    }
}
