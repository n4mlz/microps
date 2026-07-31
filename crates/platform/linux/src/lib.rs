mod os;
mod tap;

pub use os::should_terminate;
pub use tap::Tap;

#[derive(Copy, Clone, Default)]
pub struct LinuxPlatform;
