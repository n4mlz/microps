mod driver;
mod os;

pub use driver::{EtherTapDevice, SOFT_IRQ, ether_tap_irq};
pub use os::should_terminate;

#[derive(Copy, Clone, Default)]
pub struct LinuxPlatform;
