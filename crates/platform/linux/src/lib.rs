mod os;

pub use os::should_terminate;

#[derive(Copy, Clone, Default)]
pub struct LinuxPlatform;
