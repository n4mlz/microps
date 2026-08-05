use alloc::vec::Vec;

use getset::CopyGetters;

use super::{Ipv4Addr, Ipv4Interface};
use crate::{InterfaceKey, Lock, Platform};

#[derive(Debug, Clone, Copy, PartialEq, Eq, CopyGetters)]
pub struct Ipv4Route {
    #[getset(get_copy = "pub")]
    network: Ipv4Addr,
    #[getset(get_copy = "pub")]
    netmask: Ipv4Addr,
    #[getset(get_copy = "pub")]
    nexthop: Ipv4Addr,
    #[getset(get_copy = "pub")]
    interface: InterfaceKey,
}

impl Ipv4Route {
    pub const fn new(
        network: Ipv4Addr,
        netmask: Ipv4Addr,
        nexthop: Ipv4Addr,
        interface: InterfaceKey,
    ) -> Self {
        Self {
            network,
            netmask,
            nexthop,
            interface,
        }
    }

    fn prefix_len(self) -> u32 {
        u32::from_be_bytes(*self.netmask.as_bytes()).count_ones()
    }

    fn matches(self, destination: Ipv4Addr) -> bool {
        u32::from_be_bytes(*destination.as_bytes()) & u32::from_be_bytes(*self.netmask.as_bytes())
            == u32::from_be_bytes(*self.network.as_bytes())
    }
}

#[derive(Debug)]
pub struct Ipv4RoutingTable<P: Platform> {
    routes: P::Mutex<Vec<Ipv4Route>>,
}

impl<P: Platform> Default for Ipv4RoutingTable<P> {
    fn default() -> Self {
        Self {
            routes: P::Mutex::new(Vec::new()),
        }
    }
}

impl<P: Platform> Ipv4RoutingTable<P> {
    pub fn add_interface_route(&self, key: InterfaceKey, interface: Ipv4Interface) {
        let network = Ipv4Addr::from(core::array::from_fn(|index| {
            interface.unicast().as_bytes()[index] & interface.netmask().as_bytes()[index]
        }));
        self.add(Ipv4Route::new(
            network,
            interface.netmask(),
            Ipv4Addr::ANY,
            key,
        ));
    }

    pub fn set_default_gateway(&self, interface: InterfaceKey, gateway: Ipv4Addr) {
        self.add(Ipv4Route::new(
            Ipv4Addr::ANY,
            Ipv4Addr::ANY,
            gateway,
            interface,
        ));
    }

    pub fn add(&self, route: Ipv4Route) {
        self.routes
            .acquire()
            .expect("IPv4 routing table lock is infallible")
            .push(route);
    }

    pub fn lookup(&self, destination: Ipv4Addr) -> Option<Ipv4Route> {
        self.routes
            .acquire()
            .expect("IPv4 routing table lock is infallible")
            .iter()
            .copied()
            .filter(|route| route.matches(destination))
            .max_by_key(|route| route.prefix_len())
    }
}
