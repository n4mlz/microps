mod ether_tap;

pub use ether_tap::{EtherTapDevice, irq as ether_tap_irq};

pub const SOFT_IRQ: usize = libc::SIGUSR1 as usize;
