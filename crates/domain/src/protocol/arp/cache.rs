use alloc::vec::Vec;

use crate::{
    Lock, Platform,
    protocol::{Ipv4Addr, MacAddr},
};

pub const CACHE_TIMEOUT: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    protocol: Ipv4Addr,
    hardware: Option<MacAddr>,
    updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entries {
    entries: Vec<Entry>,
}

impl Entries {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl Entries {
    fn expire(&mut self, now: u64) {
        self.entries
            .retain(|entry| now.saturating_sub(entry.updated_at) <= CACHE_TIMEOUT);
    }

    fn find(&self, protocol: Ipv4Addr) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.protocol == protocol)
    }

    fn update(&mut self, protocol: Ipv4Addr, hardware: MacAddr, now: u64) -> bool {
        let Some(index) = self.find(protocol) else {
            return false;
        };
        self.entries[index].hardware = Some(hardware);
        self.entries[index].updated_at = now;
        true
    }

    fn insert_incomplete(&mut self, protocol: Ipv4Addr, now: u64) {
        if self.find(protocol).is_some() {
            return;
        }
        self.entries.push(Entry {
            protocol,
            hardware: None,
            updated_at: now,
        });
    }

    fn insert(&mut self, protocol: Ipv4Addr, hardware: MacAddr, now: u64) {
        self.entries.push(Entry {
            protocol,
            hardware: Some(hardware),
            updated_at: now,
        });
    }

    fn resolve(&mut self, protocol: Ipv4Addr, now: u64) -> Option<MacAddr> {
        self.expire(now);
        self.find(protocol)
            .and_then(|index| self.entries[index].hardware)
    }
}

pub struct ArpCache<P: Platform> {
    entries: P::Mutex<Entries>,
}

impl<P: Platform> ArpCache<P> {
    pub fn new() -> Self {
        Self {
            entries: P::Mutex::new(Entries::new()),
        }
    }

    pub fn resolve(&self, protocol: Ipv4Addr, now: u64) -> Option<MacAddr> {
        self.entries
            .acquire()
            .expect("ARP cache lock is infallible")
            .resolve(protocol, now)
    }

    pub fn update(&self, protocol: Ipv4Addr, hardware: MacAddr, now: u64) -> bool {
        let mut entries = self
            .entries
            .acquire()
            .expect("ARP cache lock is infallible");
        entries.expire(now);
        entries.update(protocol, hardware, now)
    }

    pub fn insert(&self, protocol: Ipv4Addr, hardware: MacAddr, now: u64) {
        let mut entries = self
            .entries
            .acquire()
            .expect("ARP cache lock is infallible");
        entries.expire(now);
        entries.insert(protocol, hardware, now);
    }

    pub fn insert_incomplete(&self, protocol: Ipv4Addr, now: u64) {
        let mut entries = self
            .entries
            .acquire()
            .expect("ARP cache lock is infallible");
        entries.expire(now);
        entries.insert_incomplete(protocol, now);
    }
}

impl<P: Platform> Default for ArpCache<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_entry_is_resolved_by_an_arp_update() {
        let protocol = Ipv4Addr::from([192, 0, 2, 2]);
        let hardware = MacAddr::from([2, 0, 0, 0, 0, 2]);
        let mut entries = Entries::new();

        entries.insert_incomplete(protocol, 0);
        assert_eq!(entries.resolve(protocol, 0), None);
        assert!(entries.update(protocol, hardware, 1));
        assert_eq!(entries.resolve(protocol, 1), Some(hardware));
    }
}
