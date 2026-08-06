use alloc::{collections::VecDeque, vec::Vec};

use super::{Ipv4Endpoint, TcpFlags};

const RETRANS_RTO: u64 = 200_000;
const RETRANS_DEADLINE: u64 = 12_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retrans {
    pub local: Ipv4Endpoint,
    pub remote: Ipv4Endpoint,
    pub seq: u32,
    pub ack: u32,
    pub flags: TcpFlags,
    pub window: u16,
    pub payload: Vec<u8>,
    first_sent: u64,
    last_sent: u64,
    rto: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetransQueue {
    entries: VecDeque<Retrans>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetransInfo {
    pub local: Ipv4Endpoint,
    pub remote: Ipv4Endpoint,
    pub ack: u32,
    pub window: u16,
}

#[derive(Debug)]
pub struct DueRetrans {
    pub entries: Vec<Retrans>,
    pub expired: bool,
}

impl RetransQueue {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
        }
    }

    pub fn enqueue(
        &mut self,
        seq: u32,
        flags: TcpFlags,
        payload: &[u8],
        timestamp: u64,
        info: RetransInfo,
    ) {
        self.entries.push_back(Retrans {
            local: info.local,
            remote: info.remote,
            seq,
            ack: info.ack,
            flags,
            window: info.window,
            payload: payload.into(),
            first_sent: timestamp,
            last_sent: timestamp,
            rto: RETRANS_RTO,
        });
    }

    pub fn due(&mut self, timestamp: u64, info: RetransInfo) -> DueRetrans {
        if self
            .entries
            .iter()
            .any(|entry| timestamp.saturating_sub(entry.first_sent) > RETRANS_DEADLINE)
        {
            return DueRetrans {
                entries: Vec::new(),
                expired: true,
            };
        }

        let entries = self
            .entries
            .iter_mut()
            .filter_map(|entry| {
                if timestamp.saturating_sub(entry.last_sent) < entry.rto {
                    return None;
                }
                entry.last_sent = timestamp;
                entry.rto = entry.rto.saturating_mul(2);
                entry.local = info.local;
                entry.remote = info.remote;
                entry.ack = info.ack;
                entry.window = info.window;
                Some(entry.clone())
            })
            .collect();
        DueRetrans {
            entries,
            expired: false,
        }
    }

    pub fn cleanup(&mut self, snd_una: u32) {
        while let Some(entry) = self.entries.front() {
            let consumed = entry.payload.len() as u32
                + u32::from(entry.flags.intersects(TcpFlags::SYN | TcpFlags::FIN));
            if snd_una < entry.seq.wrapping_add(consumed) {
                break;
            }
            self.entries.pop_front();
        }
    }
}

impl Default for RetransQueue {
    fn default() -> Self {
        Self::new()
    }
}
