use core::convert::Infallible;

use microps::Runtime;

use crate::LinuxPlatform;

impl Runtime for LinuxPlatform {
    type Error = Infallible;

    fn poll() -> Result<(), Self::Error> {
        Ok(())
    }

    fn shutdown() {}
}
