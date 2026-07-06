use microps::Random;
use rand::random;

use crate::LinuxPlatform;

impl Random for LinuxPlatform {
    type Error = core::convert::Infallible;

    fn random16() -> std::result::Result<u16, Self::Error> {
        Ok(random())
    }
}
