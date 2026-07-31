use crate::{Device, NetInterface};

mod ethernet;
mod ipv4;

pub use ethernet::*;
pub use ipv4::*;

impl Device {
    pub fn input(&mut self, interface: &mut dyn NetInterface, frame_type: u16, data: &[u8]) {
        if frame_type == EtherType::Ipv4 as u16 {
            interface.input(data);
        }
    }
}
