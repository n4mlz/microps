use alloc::vec::Vec;

use slotmap::{SlotMap, new_key_type};

use super::{Ipv4Endpoint, TcpPcb, TcpState};
use crate::{Lock, Platform};

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
            pcbs[pcb].bind_local(local);
            return Ok(local);
        }
        for port in crate::protocol::DYNAMIC_PORT_MIN..=crate::protocol::DYNAMIC_PORT_MAX {
            let candidate = Ipv4Endpoint::new(local.address(), port);
            if !pcbs
                .iter()
                .any(|(key, other)| key != pcb && Self::matches(other, candidate, remote))
            {
                pcbs[pcb].bind_local(candidate);
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
    #[error("no ephemeral TCP port is available")]
    NoEphemeralPort,
}

impl<P: Platform> Default for TcpPcbRegistry<P> {
    fn default() -> Self {
        Self {
            pcbs: P::Mutex::new(SlotMap::default()),
        }
    }
}
