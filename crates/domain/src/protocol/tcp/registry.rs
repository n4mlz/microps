use getset::CopyGetters;
use slotmap::{SlotMap, new_key_type};

use super::{Ipv4Endpoint, TcpState};
use crate::{Lock, Platform};

new_key_type! {
    pub struct TcpPcbKey;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub(crate) struct TcpPcb {
    #[getset(get_copy = "pub(crate)")]
    state: TcpState,
    #[getset(get_copy = "pub(crate)")]
    local: Ipv4Endpoint,
    #[getset(get_copy = "pub(crate)")]
    remote: Ipv4Endpoint,
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
            .insert(TcpPcb {
                state: TcpState::Closed,
                local: Ipv4Endpoint::new(crate::protocol::Ipv4Addr::ANY, 0),
                remote: Ipv4Endpoint::new(crate::protocol::Ipv4Addr::ANY, 0),
            })
    }

    pub fn close(&self, pcb: TcpPcbKey) -> bool {
        self.pcbs
            .acquire()
            .expect("TCP PCB registry lock is infallible")
            .remove(pcb)
            .is_some()
    }

    pub(crate) fn select(
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

impl<P: Platform> Default for TcpPcbRegistry<P> {
    fn default() -> Self {
        Self {
            pcbs: P::Mutex::new(SlotMap::default()),
        }
    }
}
