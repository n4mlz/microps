use alloc::collections::VecDeque;

use getset::CopyGetters;
use slotmap::{SlotMap, new_key_type};

use super::{Ipv4Endpoint, TcpState};
use crate::{Lock, Platform};

new_key_type! {
    pub struct TcpPcbKey;
}

const RECEIVE_WINDOW_SIZE: u16 = u16::MAX;

#[derive(Debug, Clone, PartialEq, Eq, CopyGetters)]
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
    #[getset(get_copy = "pub")]
    snd_wnd: u16,
    #[getset(get_copy = "pub")]
    snd_wl1: u32,
    #[getset(get_copy = "pub")]
    snd_wl2: u32,
    #[getset(get_copy = "pub")]
    mss: usize,
    receive_buffer: VecDeque<u8>,
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
            snd_wnd: 0,
            snd_wl1: 0,
            snd_wl2: 0,
            mss: 0,
            receive_buffer: VecDeque::new(),
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
        self.rcv_wnd = RECEIVE_WINDOW_SIZE;
        self.snd_nxt = iss.wrapping_add(1);
        self.snd_una = iss;
        self.state = TcpState::SynReceived;
    }

    pub fn accept_segment(&self, seq: u32, length: u32) -> bool {
        if length == 0 {
            if self.rcv_wnd == 0 {
                return seq == self.rcv_nxt;
            }
            return self.rcv_nxt <= seq && seq < self.rcv_nxt.wrapping_add(u32::from(self.rcv_wnd));
        }
        if self.rcv_wnd == 0 {
            return false;
        }
        let last = seq.wrapping_add(length - 1);
        let window_end = self.rcv_nxt.wrapping_add(u32::from(self.rcv_wnd));
        (self.rcv_nxt <= seq && seq < window_end) || (self.rcv_nxt <= last && last < window_end)
    }

    pub fn accept_ack(&mut self, seq: u32, ack: u32, window: u16) -> TcpAckResult {
        if self.state != TcpState::SynReceived && self.state != TcpState::Established {
            return TcpAckResult::Ignored;
        }
        let accepted = if self.state == TcpState::SynReceived {
            self.snd_una <= ack && ack <= self.snd_nxt
        } else {
            self.snd_una < ack && ack <= self.snd_nxt
        };
        if accepted {
            self.snd_una = self.snd_una.max(ack);
            if self.snd_wl1 < seq || (self.snd_wl1 == seq && self.snd_wl2 <= ack) {
                self.snd_wnd = window;
                self.snd_wl1 = seq;
                self.snd_wl2 = ack;
            }
            if self.state == TcpState::SynReceived {
                self.state = TcpState::Established;
            }
            TcpAckResult::Accepted
        } else if ack < self.snd_una {
            TcpAckResult::Old
        } else if self.snd_nxt < ack {
            TcpAckResult::TooNew
        } else {
            TcpAckResult::Unchanged
        }
    }

    pub fn accept_payload(&mut self, seq: u32, payload: &[u8]) -> bool {
        if self.rcv_nxt != seq || usize::from(self.rcv_wnd) < payload.len() {
            return false;
        }
        self.receive_buffer.extend(payload.iter().copied());
        self.rcv_nxt = self.rcv_nxt.wrapping_add(payload.len() as u32);
        self.rcv_wnd -= payload.len() as u16;
        true
    }

    pub fn receive(&mut self, buffer: &mut [u8]) -> usize {
        let length = buffer.len().min(self.receive_buffer.len());
        for byte in &mut buffer[..length] {
            *byte = self.receive_buffer.pop_front().expect("length is bounded");
        }
        self.rcv_wnd = self.rcv_wnd.saturating_add(length as u16);
        length
    }

    pub fn set_mss(&mut self, mss: usize) {
        self.mss = mss;
    }

    pub fn advance_send(&mut self, length: usize) {
        self.snd_nxt = self.snd_nxt.wrapping_add(length as u32);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpAckResult {
    Accepted,
    Old,
    TooNew,
    Unchanged,
    Ignored,
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
            .cloned()
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
                    && Self::matches(pcb, local, remote)
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

    pub fn wait_for_update(&self, pcb: TcpPcbKey) -> Result<(), TcpPcbError> {
        let pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        if !pcbs.contains_key(pcb) {
            return Err(TcpPcbError::NotFound);
        }
        let _ = self.pcbs.wait(pcbs).expect("TCP PCB wait is infallible");
        Ok(())
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
            .find(|(_, pcb)| Self::matches(pcb, local, remote))
            .map(|(key, pcb)| (key, pcb.state()))
    }

    fn matches(pcb: &TcpPcb, local: Ipv4Endpoint, remote: Ipv4Endpoint) -> bool {
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
