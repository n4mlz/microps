use std::sync::Mutex;

use microps::Lock;

use crate::LinuxPlatform;

static LOCK: Mutex<()> = Mutex::new(());

impl Lock for LinuxPlatform {
    type Error = core::convert::Infallible;
    type Guard = std::sync::MutexGuard<'static, ()>;

    fn acquire() -> Result<Self::Guard, Self::Error> {
        Ok(match LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        })
    }
}
