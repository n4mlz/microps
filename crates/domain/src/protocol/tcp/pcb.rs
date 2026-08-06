use alloc::collections::VecDeque;

use getset::CopyGetters;

use super::{Ipv4Endpoint, TcpFlags, TcpState};
use crate::protocol::tcp::retrans::{Retrans, RetransInfo, RetransQueue};

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
    retrans_queue: RetransQueue,
}

impl TcpPcb {
    // Construction and connection setup.
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
            retrans_queue: RetransQueue::new(),
        }
    }

    pub fn listen(&mut self, local: Ipv4Endpoint, remote: Ipv4Endpoint) {
        self.local = local;
        self.remote = remote;
        self.state = TcpState::Listen;
    }

    pub fn bind_local(&mut self, local: Ipv4Endpoint) {
        self.local = local;
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

    pub fn connect(&mut self, local: Ipv4Endpoint, remote: Ipv4Endpoint, iss: u32) {
        self.local = local;
        self.remote = remote;
        self.iss = iss;
        self.rcv_wnd = RECEIVE_WINDOW_SIZE;
        self.snd_nxt = iss.wrapping_add(1);
        self.snd_una = iss;
        self.state = TcpState::SynSent;
    }

    pub fn accept_syn_ack(&mut self, seq: u32, ack: u32, window: u16) {
        self.rcv_nxt = seq.wrapping_add(1);
        if self.snd_una < ack && ack <= self.snd_nxt {
            self.snd_una = ack;
            self.cleanup_retrans();
            self.snd_wnd = window;
            self.snd_wl1 = seq;
            self.snd_wl2 = ack;
            self.state = TcpState::Established;
        }
    }

    // Segment validation and state transitions.
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
        if !matches!(
            self.state,
            TcpState::SynReceived | TcpState::Established | TcpState::CloseWait
        ) {
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
            self.cleanup_retrans();
            TcpAckResult::Accepted
        } else if ack < self.snd_una {
            TcpAckResult::Old
        } else if self.snd_nxt < ack {
            TcpAckResult::TooNew
        } else {
            TcpAckResult::Unchanged
        }
    }

    pub fn accept_fin(&mut self, seq: u32, payload_len: usize) -> bool {
        if self.rcv_nxt != seq.wrapping_add(payload_len as u32) {
            return false;
        }
        self.rcv_nxt = self.rcv_nxt.wrapping_add(1);
        if matches!(self.state, TcpState::SynReceived | TcpState::Established) {
            self.state = TcpState::CloseWait;
        }
        true
    }

    pub fn enter_last_ack(&mut self) {
        self.advance_send(1);
        self.state = TcpState::LastAck;
    }

    // Sequence numbers, windows, and user data.
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

    pub fn has_received_data(&self) -> bool {
        !self.receive_buffer.is_empty()
    }

    pub fn set_mss(&mut self, mss: usize) {
        self.mss = mss;
    }

    pub fn advance_send(&mut self, length: usize) {
        self.snd_nxt = self.snd_nxt.wrapping_add(length as u32);
    }

    // Retransmission queue integration.
    pub fn queue_retrans(&mut self, seq: u32, flags: TcpFlags, payload: &[u8], timestamp: u64) {
        let info = RetransInfo {
            local: self.local,
            remote: self.remote,
            ack: self.rcv_nxt,
            window: self.rcv_wnd,
        };
        self.retrans_queue
            .enqueue(seq, flags, payload, timestamp, info);
    }

    pub fn due_retrans(&mut self, timestamp: u64) -> alloc::vec::Vec<Retrans> {
        let info = RetransInfo {
            local: self.local,
            remote: self.remote,
            ack: self.rcv_nxt,
            window: self.rcv_wnd,
        };
        let result = self.retrans_queue.due(timestamp, info);
        if result.expired {
            self.state = TcpState::Closed;
        }
        result.entries
    }

    fn cleanup_retrans(&mut self) {
        self.retrans_queue.cleanup(self.snd_una);
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
