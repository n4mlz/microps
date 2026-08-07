use alloc::collections::VecDeque;

use getset::{CopyGetters, Getters, MutGetters, Setters};

use super::{Ipv4Endpoint, TcpFlags, TcpPcbKey, TcpState};
use crate::protocol::tcp::retrans::{Retrans, RetransInfo, RetransQueue};

const RECEIVE_WINDOW_SIZE: u16 = u16::MAX;
pub const TIME_WAIT_SECONDS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TcpSegment<'a> {
    pub(crate) seq: u32,
    pub(crate) ack: u32,
    pub(crate) length: u32,
    pub(crate) window: u16,
    pub(crate) flags: TcpFlags,
    pub(crate) data: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TcpResponse {
    FromPcb(TcpFlags),
    Reset { seq: u32, ack: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TcpPcbEvent {
    None,
    AcceptSyn { seq: u32 },
    Established,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TcpPcbAction {
    pub(crate) response: Option<TcpResponse>,
    pub(crate) event: TcpPcbEvent,
}

impl TcpPcbAction {
    fn none() -> Self {
        Self {
            response: None,
            event: TcpPcbEvent::None,
        }
    }

    fn respond(flags: TcpFlags) -> Self {
        Self {
            response: Some(TcpResponse::FromPcb(flags)),
            event: TcpPcbEvent::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, CopyGetters, Getters, MutGetters, Setters)]
pub struct TcpPcb {
    #[getset(get_copy = "pub")]
    state: TcpState,
    #[getset(get_copy = "pub")]
    #[getset(set = "pub")]
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
    #[getset(set = "pub")]
    mss: usize,
    receive_buffer: VecDeque<u8>,
    retrans_queue: RetransQueue,
    time_wait_until: Option<u64>,
    #[getset(get_copy = "pub", set = "pub")]
    parent: Option<TcpPcbKey>,
    #[getset(get = "pub", get_mut = "pub")]
    backlog: VecDeque<TcpPcbKey>,
    #[getset(get_copy = "pub", set = "pub")]
    backlog_max: usize,
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
            time_wait_until: None,
            parent: None,
            backlog: VecDeque::new(),
            backlog_max: 0,
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

    pub fn accept_simultaneous_syn(&mut self, seq: u32) {
        self.rcv_nxt = seq.wrapping_add(1);
        self.state = TcpState::SynReceived;
    }

    pub(crate) fn on_segment(&mut self, segment: &TcpSegment<'_>, timestamp: u64) -> TcpPcbAction {
        match self.state {
            TcpState::Closed => TcpPcbAction::none(),
            TcpState::Listen => self.on_listen(segment),
            TcpState::SynSent => self.on_syn_sent(segment),
            TcpState::SynReceived
            | TcpState::Established
            | TcpState::FinWait1
            | TcpState::FinWait2
            | TcpState::CloseWait
            | TcpState::Closing
            | TcpState::LastAck
            | TcpState::TimeWait => self.on_connected(segment, timestamp),
        }
    }

    fn on_listen(&self, segment: &TcpSegment<'_>) -> TcpPcbAction {
        if segment.flags.contains(TcpFlags::ACK) {
            return TcpPcbAction {
                response: Some(TcpResponse::Reset {
                    seq: segment.ack,
                    ack: 0,
                }),
                event: TcpPcbEvent::None,
            };
        }
        if segment.flags.contains(TcpFlags::SYN) {
            return TcpPcbAction {
                response: None,
                event: TcpPcbEvent::AcceptSyn { seq: segment.seq },
            };
        }
        TcpPcbAction::none()
    }

    fn on_syn_sent(&mut self, segment: &TcpSegment<'_>) -> TcpPcbAction {
        let ack_acceptable = if segment.flags.contains(TcpFlags::ACK) {
            if segment.ack <= self.iss || segment.ack > self.snd_nxt {
                return TcpPcbAction {
                    response: Some(TcpResponse::Reset {
                        seq: segment.ack,
                        ack: 0,
                    }),
                    event: TcpPcbEvent::None,
                };
            }
            true
        } else {
            false
        };

        if segment.flags.contains(TcpFlags::RST) {
            return TcpPcbAction {
                response: None,
                event: if ack_acceptable {
                    TcpPcbEvent::Close
                } else {
                    TcpPcbEvent::None
                },
            };
        }
        if !segment.flags.contains(TcpFlags::SYN) {
            return TcpPcbAction::none();
        }

        if ack_acceptable {
            self.accept_syn_ack(segment.seq, segment.ack, segment.window);
            if self.state == TcpState::Established {
                return TcpPcbAction {
                    response: Some(TcpResponse::FromPcb(TcpFlags::ACK)),
                    event: TcpPcbEvent::Established,
                };
            }
        }

        self.accept_simultaneous_syn(segment.seq);
        TcpPcbAction::respond(TcpFlags::SYN | TcpFlags::ACK)
    }

    fn on_connected(&mut self, segment: &TcpSegment<'_>, timestamp: u64) -> TcpPcbAction {
        let state = self.state;
        if !self.accept_segment(segment.seq, segment.length) {
            return if !segment.flags.contains(TcpFlags::RST) {
                TcpPcbAction::respond(TcpFlags::ACK)
            } else {
                TcpPcbAction::none()
            };
        }
        if segment.flags.contains(TcpFlags::RST) {
            return TcpPcbAction {
                response: None,
                event: TcpPcbEvent::Close,
            };
        }
        if segment.flags.contains(TcpFlags::SYN) {
            return TcpPcbAction {
                response: Some(TcpResponse::FromPcb(TcpFlags::RST)),
                event: TcpPcbEvent::Close,
            };
        }
        if state == TcpState::LastAck
            && segment.flags.contains(TcpFlags::ACK)
            && segment.ack == self.snd_nxt
        {
            return TcpPcbAction {
                response: None,
                event: TcpPcbEvent::Close,
            };
        }
        if !segment.flags.contains(TcpFlags::ACK) {
            return TcpPcbAction::none();
        }

        match self.accept_ack(segment.seq, segment.ack, segment.window) {
            TcpAckResult::TooNew => return TcpPcbAction::respond(TcpFlags::ACK),
            TcpAckResult::Accepted
            | TcpAckResult::Old
            | TcpAckResult::Unchanged
            | TcpAckResult::Ignored => {}
        }

        let became_established =
            state == TcpState::SynReceived && self.state == TcpState::Established;
        if state == TcpState::Closing && segment.ack == self.snd_nxt {
            self.enter_time_wait(timestamp);
        }

        let mut action = TcpPcbAction {
            response: None,
            event: if became_established {
                TcpPcbEvent::Established
            } else {
                TcpPcbEvent::None
            },
        };
        if matches!(
            state,
            TcpState::Established | TcpState::FinWait1 | TcpState::FinWait2
        ) && !segment.data.is_empty()
        {
            if !self.accept_payload(segment.seq, segment.data) {
                return TcpPcbAction::respond(TcpFlags::ACK);
            }
            action.response = Some(TcpResponse::FromPcb(TcpFlags::ACK));
        }
        if segment.flags.contains(TcpFlags::FIN)
            && matches!(
                state,
                TcpState::SynReceived
                    | TcpState::Established
                    | TcpState::FinWait1
                    | TcpState::FinWait2
                    | TcpState::CloseWait
                    | TcpState::Closing
                    | TcpState::LastAck
                    | TcpState::TimeWait
            )
        {
            if !self.accept_fin_at(segment.seq, segment.data.len(), timestamp) {
                return TcpPcbAction::respond(TcpFlags::ACK);
            }
            if state == TcpState::FinWait1 {
                if segment.ack == self.snd_nxt {
                    self.enter_time_wait(timestamp);
                } else {
                    self.set_closing();
                }
            }
            action.response = Some(TcpResponse::FromPcb(TcpFlags::ACK));
        }
        action
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
            TcpState::SynReceived
                | TcpState::Established
                | TcpState::CloseWait
                | TcpState::FinWait1
                | TcpState::FinWait2
                | TcpState::Closing
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
            } else if self.state == TcpState::FinWait1 && ack == self.snd_nxt {
                self.state = TcpState::FinWait2;
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
        self.accept_fin_at(seq, payload_len, 0)
    }

    pub fn accept_fin_at(&mut self, seq: u32, payload_len: usize, timestamp: u64) -> bool {
        if self.rcv_nxt != seq.wrapping_add(payload_len as u32) {
            return false;
        }
        self.rcv_nxt = self.rcv_nxt.wrapping_add(1);
        match self.state {
            TcpState::SynReceived | TcpState::Established => self.state = TcpState::CloseWait,
            TcpState::FinWait2 => self.enter_time_wait(timestamp),
            TcpState::TimeWait => self.restart_time_wait(timestamp),
            _ => {}
        }
        true
    }

    pub fn enter_fin_wait1(&mut self) {
        self.advance_send(1);
        self.state = TcpState::FinWait1;
    }

    pub fn set_closing(&mut self) {
        self.state = TcpState::Closing;
    }

    pub fn enter_last_ack(&mut self) {
        self.advance_send(1);
        self.state = TcpState::LastAck;
    }

    pub fn enter_time_wait(&mut self, timestamp: u64) {
        self.state = TcpState::TimeWait;
        self.restart_time_wait(timestamp);
    }

    pub fn restart_time_wait(&mut self, timestamp: u64) {
        self.time_wait_until = Some(timestamp.saturating_add(TIME_WAIT_SECONDS));
    }

    pub fn time_wait_expired(&self, timestamp: u64) -> bool {
        self.state == TcpState::TimeWait
            && self.time_wait_until.is_some_and(|until| timestamp > until)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Ipv4Addr;

    fn connected_pcb() -> TcpPcb {
        let local = Ipv4Endpoint::new(Ipv4Addr::ANY, 7);
        let remote = Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 50000);
        let mut pcb = TcpPcb::new();
        pcb.accept_syn(local, remote, 100, 200);
        pcb.accept_ack(101, 201, 4096);
        pcb
    }

    #[test]
    fn syn_received_ack_becomes_established() {
        let mut pcb = TcpPcb::new();
        pcb.accept_syn(
            Ipv4Endpoint::new(Ipv4Addr::ANY, 7),
            Ipv4Endpoint::new(Ipv4Addr::from([192, 0, 2, 2]), 50000),
            100,
            200,
        );
        let segment = TcpSegment {
            seq: 101,
            ack: 201,
            length: 0,
            window: 4096,
            flags: TcpFlags::ACK,
            data: &[],
        };

        let action = pcb.on_segment(&segment, 0);

        assert_eq!(pcb.state(), TcpState::Established);
        assert_eq!(action.response, None);
        assert_eq!(action.event, TcpPcbEvent::Established);
    }

    #[test]
    fn payload_and_fin_are_processed_in_one_segment() {
        let mut pcb = connected_pcb();
        let segment = TcpSegment {
            seq: 101,
            ack: 201,
            length: 4,
            window: 4096,
            flags: TcpFlags::ACK | TcpFlags::FIN,
            data: b"hey",
        };

        let action = pcb.on_segment(&segment, 0);

        assert_eq!(pcb.state(), TcpState::CloseWait);
        assert_eq!(pcb.rcv_nxt(), 105);
        assert_eq!(action.response, Some(TcpResponse::FromPcb(TcpFlags::ACK)));
        assert!(pcb.has_received_data());
    }

    #[test]
    fn last_ack_is_closed_by_the_final_ack() {
        let mut pcb = connected_pcb();
        pcb.enter_last_ack();
        let segment = TcpSegment {
            seq: 101,
            ack: pcb.snd_nxt(),
            length: 0,
            window: 4096,
            flags: TcpFlags::ACK,
            data: &[],
        };

        let action = pcb.on_segment(&segment, 0);

        assert_eq!(action.response, None);
        assert_eq!(action.event, TcpPcbEvent::Close);
    }
}
