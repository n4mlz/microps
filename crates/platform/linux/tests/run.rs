use core::mem::size_of;

use linux::LinuxPlatform;

#[test]
fn linux_platform_is_stateless() {
    assert_eq!(size_of::<LinuxPlatform>(), 0);
}
