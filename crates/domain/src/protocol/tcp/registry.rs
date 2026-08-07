use alloc::vec::Vec;

use slotmap::{SlotMap, new_key_type};

use super::{Ipv4Endpoint, TcpPcb, TcpState};
use crate::{Lock, Platform, WaitResult};

new_key_type! {
    pub struct TcpPcbKey;
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

    pub fn bind(&self, pcb: TcpPcbKey, local: Ipv4Endpoint) -> Result<(), TcpPcbError> {
        if local.port() == 0 {
            return Err(TcpPcbError::InvalidEndpoint);
        }
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let current = pcbs.get(pcb).ok_or(TcpPcbError::NotFound)?;
        if current.state() != TcpState::Closed {
            return Err(TcpPcbError::InvalidState(current.state()));
        }
        if pcbs.iter().any(|(key, other)| {
            key != pcb
                && other.state() != TcpState::Closed
                && (Self::matches(
                    other,
                    local,
                    Ipv4Endpoint::new(crate::protocol::Ipv4Addr::ANY, 0),
                ) || (other.local().address() == crate::protocol::Ipv4Addr::ANY
                    && other.local().port() == local.port()))
        }) {
            return Err(TcpPcbError::AlreadyBound);
        }
        pcbs[pcb].set_local(local);
        Ok(())
    }

    pub fn listen(&self, pcb: TcpPcbKey, backlog: usize) -> Result<(), TcpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let current = pcbs.get_mut(pcb).ok_or(TcpPcbError::NotFound)?;
        if current.state() != TcpState::Closed {
            return Err(TcpPcbError::InvalidState(current.state()));
        }
        if current.local().port() == 0 {
            return Err(TcpPcbError::InvalidEndpoint);
        }
        current.set_backlog_max(backlog);
        current.listen(
            current.local(),
            Ipv4Endpoint::new(crate::protocol::Ipv4Addr::ANY, 0),
        );
        self.pcbs.wake_all();
        Ok(())
    }

    pub fn allocate_child(
        &self,
        listener: TcpPcbKey,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
        seq: u32,
        iss: u32,
    ) -> Result<TcpPcbKey, TcpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let listener_pcb = pcbs.get(listener).ok_or(TcpPcbError::NotFound)?;
        if listener_pcb.state() != TcpState::Listen {
            return Err(TcpPcbError::InvalidState(listener_pcb.state()));
        }
        if listener_pcb.backlog().len() >= listener_pcb.backlog_max() {
            return Err(TcpPcbError::BacklogFull);
        }
        let mut child = TcpPcb::new();
        child.set_parent(Some(listener));
        child.accept_syn(local, remote, seq, iss);
        Ok(pcbs.insert(child))
    }

    pub fn enqueue_established(&self, pcb: TcpPcbKey) -> Result<(), TcpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let parent = pcbs
            .get(pcb)
            .ok_or(TcpPcbError::NotFound)?
            .parent()
            .ok_or(TcpPcbError::InvalidState(TcpState::Established))?;
        let parent_pcb = pcbs.get_mut(parent).ok_or(TcpPcbError::NotFound)?;
        if !parent_pcb.backlog().contains(&pcb) {
            parent_pcb.backlog_mut().push_back(pcb);
        }
        self.pcbs.wake_all();
        Ok(())
    }

    pub fn accept(&self, listener: TcpPcbKey) -> Result<TcpPcbKey, TcpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        loop {
            let listener_pcb = pcbs.get_mut(listener).ok_or(TcpPcbError::NotFound)?;
            if listener_pcb.state() != TcpState::Listen {
                return Err(TcpPcbError::InvalidState(listener_pcb.state()));
            }
            if let Some(child) = listener_pcb.backlog_mut().pop_front() {
                return Ok(child);
            }
            pcbs = match self.pcbs.wait(pcbs).expect("TCP PCB wait is infallible") {
                WaitResult::Notified(pcbs) => pcbs,
                WaitResult::Interrupted(_) => return Err(TcpPcbError::Interrupted),
            };
        }
    }

    pub fn close(&self, pcb: TcpPcbKey) -> bool {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let Some(mut removed) = pcbs.remove(pcb) else {
            self.pcbs.wake_all();
            return false;
        };
        for child in removed.backlog_mut().drain(..) {
            pcbs.remove(child);
        }
        for parent in pcbs.values_mut() {
            parent.backlog_mut().retain(|child| *child != pcb);
        }
        self.pcbs.wake_all();
        true
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

    pub fn due_retrans(&self, timestamp: u64) -> Vec<super::Retrans> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let mut closed = false;
        let retrans = pcbs
            .values_mut()
            .flat_map(|pcb| {
                let state = pcb.state();
                let retrans = pcb.due_retrans(timestamp);
                closed |= state != TcpState::Closed && pcb.state() == TcpState::Closed;
                retrans
            })
            .collect();
        if closed {
            self.pcbs.wake_all();
        }
        retrans
    }

    pub fn expire_time_wait(&self, timestamp: u64) {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        let expired: Vec<_> = pcbs
            .iter()
            .filter(|(_, pcb)| pcb.time_wait_expired(timestamp))
            .map(|(key, _)| key)
            .collect();
        if expired.is_empty() {
            return;
        }
        for key in expired {
            pcbs.remove(key);
        }
        self.pcbs.wake_all();
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

    pub fn assign_dynamic_port(
        &self,
        pcb: TcpPcbKey,
        local: Ipv4Endpoint,
        remote: Ipv4Endpoint,
    ) -> Result<Ipv4Endpoint, TcpPcbError> {
        let mut pcbs = self
            .pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible");
        if !pcbs.contains_key(pcb) {
            return Err(TcpPcbError::NotFound);
        }
        if local.port() != 0 {
            if pcbs
                .iter()
                .any(|(key, other)| key != pcb && Self::matches(other, local, remote))
            {
                return Err(TcpPcbError::AlreadyBound);
            }
            pcbs[pcb].set_local(local);
            return Ok(local);
        }
        for port in crate::protocol::DYNAMIC_PORT_MIN..=crate::protocol::DYNAMIC_PORT_MAX {
            let candidate = Ipv4Endpoint::new(local.address(), port);
            if !pcbs
                .iter()
                .any(|(key, other)| key != pcb && Self::matches(other, candidate, remote))
            {
                pcbs[pcb].set_local(candidate);
                return Ok(candidate);
            }
        }
        Err(TcpPcbError::NoEphemeralPort)
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
            pcbs = match self.pcbs.wait(pcbs).expect("TCP PCB wait is infallible") {
                WaitResult::Notified(pcbs) => pcbs,
                WaitResult::Interrupted(_) => return Err(TcpPcbError::Interrupted),
            };
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
        if matches!(
            self.pcbs.wait(pcbs).expect("TCP PCB wait is infallible"),
            WaitResult::Interrupted(_)
        ) {
            return Err(TcpPcbError::Interrupted);
        }
        Ok(())
    }

    pub fn interrupt_all(&self) {
        self.pcbs.interrupt_all();
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
            .fold(None, |listener, (key, pcb)| {
                if !Self::matches(pcb, local, remote) {
                    return listener;
                }
                if pcb.state() != TcpState::Listen {
                    Some((key, pcb.state()))
                } else {
                    listener.or(Some((key, pcb.state())))
                }
            })
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
    #[error("wait interrupted")]
    Interrupted,
    #[error("TCP PCB does not exist")]
    NotFound,
    #[error("TCP endpoint is already in use")]
    AlreadyBound,
    #[error("no ephemeral TCP port is available")]
    NoEphemeralPort,
    #[error("TCP PCB is in an invalid state: {0:?}")]
    InvalidState(TcpState),
    #[error("TCP endpoint is invalid")]
    InvalidEndpoint,
    #[error("TCP listen backlog is full")]
    BacklogFull,
}

impl<P: Platform> Default for TcpPcbRegistry<P> {
    fn default() -> Self {
        Self {
            pcbs: P::Mutex::new(SlotMap::default()),
        }
    }
}
