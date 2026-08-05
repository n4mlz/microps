use std::sync::OnceLock;

use microps::Stack;

mod driver;
mod os;

pub use driver::{EtherTapDevice, Tap, ether_tap_irq};
pub use os::should_terminate;

#[derive(Copy, Clone, Default)]
pub struct LinuxPlatform;

static STACK: OnceLock<Stack<LinuxPlatform>> = OnceLock::new();

pub fn stack() -> &'static Stack<LinuxPlatform> {
    STACK.get_or_init(Stack::new)
}
